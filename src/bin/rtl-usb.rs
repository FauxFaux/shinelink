use anyhow::{Context, Result, bail};

use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime};
use std::{fs, io, thread};

use ctrlc;
use itertools::Itertools;
use log::{error, info};
use num_complex::Complex;
use rtlsdr_rs::error::RtlsdrError;
use rtlsdr_rs::{DEFAULT_BUF_LENGTH, RtlSdr, error::Result as RtlResult};
use rustfft::num_traits::Zero;
use rustfft::{Fft, FftPlanner};

// Radio and demodulation config
const FREQUENCY: u32 = 434_200_000;
const SAMPLE_RATE: u32 = 288_000;

// RTL Device Index
const RTL_INDEX: usize = 0;

fn main() -> Result<()> {
    // Printing to stdout will break audio output, so use this to log to stderr instead
    stderrlog::new().verbosity(log::Level::Info).init()?;

    // Shutdown flag that is set true when ctrl-c signal caught
    static SHUTDOWN: AtomicBool = AtomicBool::new(false);
    ctrlc::set_handler(|| {
        SHUTDOWN.swap(true, Ordering::Relaxed);
    })?;

    // Channel to pass receive data from receiver thread to processor thread
    let (tx, rx) = mpsc::channel();

    // Spawn thread to receive data from Radio
    let receive_thread = thread::spawn(|| {
        receive(
            &SHUTDOWN,
            RadioConfig {
                capture_freq: FREQUENCY,
                capture_rate: SAMPLE_RATE,
            },
            tx,
        )
    });

    // Spawn thread to process data and output to stdout
    let process_thread = thread::spawn(|| process_dump(&SHUTDOWN, ProcessConfig {}, rx));

    // Wait for threads to finish
    receive_thread.join().unwrap()?;
    process_thread.join().unwrap()?;

    Ok(())
}

struct RadioConfig {
    capture_freq: u32,
    capture_rate: u32,
}

struct DropCloseRtlSdr {
    sdr: Option<RtlSdr>,
}

impl DropCloseRtlSdr {
    fn new(sdr: RtlSdr) -> Self {
        DropCloseRtlSdr { sdr: Some(sdr) }
    }

    fn close(mut self) -> Result<()> {
        if let Some(mut sdr) = self.sdr.take() {
            sdr.close().map_err(from_rtl)?;
        }
        Ok(())
    }
}

impl AsMut<RtlSdr> for DropCloseRtlSdr {
    fn as_mut(&mut self) -> &mut RtlSdr {
        self.sdr.as_mut().expect("RtlSdr is None")
    }
}

impl Deref for DropCloseRtlSdr {
    type Target = RtlSdr;

    fn deref(&self) -> &Self::Target {
        self.sdr.as_ref().expect("RtlSdr is None")
    }
}

impl DerefMut for DropCloseRtlSdr {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.sdr.as_mut().expect("RtlSdr is None")
    }
}

impl Drop for DropCloseRtlSdr {
    fn drop(&mut self) {
        if let Some(ref mut sdr) = self.sdr {
            if let Err(e) = sdr.close() {
                error!("Failed to close RTL-SDR device: {:?}", e);
            }
        }
    }
}

/// Thread to open SDR device and send received data to the demod thread until
/// SHUTDOWN flag is set to true.
fn receive(shutdown: &AtomicBool, radio_config: RadioConfig, tx: Sender<Vec<u8>>) -> Result<()> {
    // Open device
    let mut sdr = DropCloseRtlSdr::new(
        RtlSdr::open(RTL_INDEX)
            .map_err(from_rtl)
            .with_context(|| "Failed to open device")?,
    );
    // Config receiver
    config_sdr(
        &mut sdr,
        radio_config.capture_freq,
        radio_config.capture_rate,
    )
    .unwrap();

    info!("Tuned to {} Hz.\n", sdr.get_center_freq());
    info!(
        "Buffer size: {}ms",
        1000.0 * 0.5 * DEFAULT_BUF_LENGTH as f32 / radio_config.capture_rate as f32
    );
    info!("Sampling at {} S/s", sdr.get_sample_rate());

    info!("Reading samples in sync mode...");
    loop {
        // Check if SHUTDOWN flag is true and break out of the loop if so
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        // Allocate a buffer to store received data
        let mut buf: Box<[u8; DEFAULT_BUF_LENGTH]> = Box::new([0; DEFAULT_BUF_LENGTH]);
        // Receive data from SDR device
        let len = sdr
            .read_sync(&mut *buf)
            .map_err(from_rtl)
            .with_context(|| "read error")?;

        if len < DEFAULT_BUF_LENGTH {
            bail!("Short read ({:#?}), samples lost, exiting!", len);
        }

        // Send received data through the channel to the processor thread
        tx.send(buf.to_vec())?;
    }

    // Shut down the device and exit
    info!("Close");
    sdr.close()?;

    Ok(())
}

fn process_dump(
    shutdown: &AtomicBool,
    _config: ProcessConfig,
    rx: Receiver<Vec<u8>>,
) -> Result<()> {
    let mut out_file = fs::File::create("dump.cu8")?;
    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        // Wait for data from the channel
        let buf = rx.recv()?;

        out_file.write_all(&buf)?;
    }

    out_file.flush()?;

    Ok(())
}

struct ProcessConfig {}

struct Process {
    _config: ProcessConfig,
    fft: Arc<dyn Fft<f32>>,
    fft_scratch: Box<[Complex<f32>]>,
    fft_output: Box<[Complex<f32>]>,
    f: io::BufWriter<fs::File>,
}

fn process_host(shutdown: &AtomicBool, config: ProcessConfig, rx: Receiver<Vec<u8>>) -> Result<()> {
    let fft = FftPlanner::new().plan_fft_forward(256);
    let mut process = Process {
        _config: config,
        fft_scratch: vec![Complex::zero(); fft.get_immutable_scratch_len()].into_boxed_slice(),
        fft_output: vec![Complex::zero(); fft.len()].into_boxed_slice(),
        fft,
        f: io::BufWriter::new(fs::File::create("../../f.cf32")?),
    };

    let mut total_time: Duration = Duration::new(0, 0);
    let mut loop_count: u64 = 0;
    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        // Wait for data from the channel
        let buf = rx.recv()?;
        let start_time = Instant::now();

        process_inner(&mut process, &buf)?;

        total_time += start_time.elapsed();
        loop_count += 1;
    }

    // Print the final average loop time when shutting down
    if loop_count > 0 {
        let final_avg_time = total_time.as_nanos() / loop_count as u128;
        info!(
            "Average processing time: {:.2?}ms ({:?} loops)",
            final_avg_time as f32 / 1.0e6,
            loop_count
        );
    }

    Ok(())
}

fn process_inner(process: &mut Process, buf: &[u8]) -> Result<()> {
    let mut buf: Vec<Complex<f32>> = buf
        .chunks_exact(2)
        .map(|chunk| Complex::new(f32::from(chunk[0]) / 256., f32::from(chunk[1])) / 256.)
        .collect();
    assert!(buf.iter().all(|v| v.is_finite()));

    let lines = (buf.len() - process.fft.len()) / 32;

    // let mut maxes = Vec::with_capacity(lines);

    let mut any_fun = false;

    for i in 0..lines {
        let start = i * 32;
        let sub = &mut buf[start..start + process.fft.len()];
        assert_eq!(process.fft.len(), sub.len());

        process.fft.process_immutable_with_scratch(
            sub,
            &mut process.fft_output,
            &mut process.fft_scratch,
        );

        let median = sub
            .iter()
            .skip(1)
            .map(|v| v.norm())
            .sorted_by(|a, b| f32::total_cmp(a, b))
            .nth(sub.len() / 2)
            .unwrap_or(0.0);

        let (max_pos, v) = sub
            .iter()
            .skip(1)
            .map(|v| v.norm())
            .enumerate()
            .max_by(|(_, a), (_, b)| f32::total_cmp(&a, &b))
            .expect("sub.len() > 0");

        let powah = v / median;
        if powah > 1.3 {
            println!("{max_pos:3} {:.2}", powah);
            any_fun = true;
        }
    }

    for v in &buf {
        process.f.write_all(&v.re.to_le_bytes())?;
        process.f.write_all(&v.im.to_le_bytes())?;
    }
    process.f.flush()?;

    if any_fun {
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
        let name = format!("{}.{:03}.cf32", now.as_secs(), now.subsec_millis());
        println!("-> {}", name);
        let mut f = io::BufWriter::new(fs::File::create(name)?);
        for v in buf {
            f.write_all(&v.re.to_le_bytes())?;
            f.write_all(&v.im.to_le_bytes())?;
        }
        f.flush()?
    }

    Ok(())
}

/// Configure the SDR device for a given receive frequency and sample rate.
fn config_sdr(sdr: &mut RtlSdr, freq: u32, rate: u32) -> RtlResult<()> {
    sdr.set_tuner_gain(rtlsdr_rs::TunerGain::Auto)?;
    sdr.set_bias_tee(false)?;
    // Reset the endpoint before we try to read from it (mandatory)
    sdr.reset_buffer()?;
    sdr.set_center_freq(freq)?;
    sdr.set_sample_rate(rate)?;

    Ok(())
}

fn from_rtl(err: RtlsdrError) -> anyhow::Error {
    match err {
        RtlsdrError::Usb(e) => e.into(),
        RtlsdrError::RtlsdrErr(e) => anyhow::anyhow!("RtlsdrErr({e:?})"),
    }
}
