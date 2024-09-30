use anyhow::Result;
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::{
    env,
    fs::File,
    process::Command,
    sync::{mpsc, Arc, Mutex},
};
use vosk::{Model, Recognizer};

struct AudioConfig {
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
}

impl AudioConfig {
    fn new(sample_rate: u32, channels: u16, bits_per_sample: u16) -> Self {
        AudioConfig {
            sample_rate,
            channels,
            bits_per_sample,
        }
    }
}

fn extract_audio(video_path: &str, audio_output: &str) -> Result<(), std::io::Error> {
    Command::new("ffmpeg")
        .args(&["-i", video_path, "-q:a", "0", "-map", "a", audio_output])
        .output()?;
    Ok(())
}

fn read_wav_samples(file_path: &str) -> Result<(AudioConfig, Vec<i16>)> {
    let mut reader = WavReader::open(file_path)?;
    let audio_spec = reader.spec();
    let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
    let conf = AudioConfig::new(
        audio_spec.sample_rate,
        audio_spec.channels,
        audio_spec.bits_per_sample,
    );
    Ok((conf, samples))
}

fn main() -> anyhow::Result<()> {
    let mut args = env::args();
    args.next();
    let model_path = args.next().expect("Model path not found");
    let video_path = args.next().expect("Video path not found");
    let output_path = args.next().expect("Output path not found");

    extract_audio(&video_path, "/home/shastri/tmp/censor-flow.wav")?;
    let (au_cfg, audio_wave) = read_wav_samples("/home/shastri/tmp/censor-flow.wav")?;

    let model = Arc::new(Model::new(&model_path).unwrap());
    let recognizer = Arc::new(Mutex::new(
        Recognizer::new(&model, au_cfg.sample_rate as f32).unwrap(),
    ));

    let spec = WavSpec {
        sample_format: SampleFormat::Int,
        channels: au_cfg.channels,
        sample_rate: au_cfg.sample_rate,
        bits_per_sample: au_cfg.bits_per_sample,
    };
    let output_file = File::create(output_path)?;
    let mut wav_writer = WavWriter::new(output_file, spec)?;

    let (tx, rx) = mpsc::channel::<Vec<i16>>();


    Ok(())
}

