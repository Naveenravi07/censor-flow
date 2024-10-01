use anyhow::Result;
use hound::{SampleFormat, WavReader, WavSpec};
use std::{env, i16, process::Command, sync::Arc, usize};
use vosk::{Model, Recognizer};

const BUFFER_LEN: usize = 8192;

#[derive(Debug)]
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
        .args(&[
            "-i",
            video_path,
            "-acodec",
            "pcm_s16le",
            "-ac",
            "1",
            "-ar",
            "16000",
            audio_output,
        ])
        .output()?;
    Ok(())
}

fn get_audio_config(file_path: &str) -> Result<AudioConfig> {
    let reader = WavReader::open(file_path)?;
    let audio_spec = reader.spec();
    let conf = AudioConfig::new(
        audio_spec.sample_rate,
        audio_spec.channels,
        audio_spec.bits_per_sample,
    );
    Ok(conf)
}

fn process_audio_in_chunks<F>(file_path: &str, mut cb: F) -> Result<()>
where
    F: FnMut(&Vec<i16>) -> Result<()>,
{
    let mut buff: Vec<i16> = Vec::with_capacity(BUFFER_LEN);
    let mut reader = WavReader::open(file_path)?;

    for sample in reader.samples::<i16>() {
        let sample = sample?;
        buff.push(sample);

        if buff.len() == BUFFER_LEN {
            let _ = cb(&buff);
            buff.clear();
        }
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let mut args = env::args();
    args.next();
    let model_path = args.next().expect("Model path not found");
    let video_path = args.next().expect("Video path not found");
    let output_audio_path = args.next().expect("Output path not found");

    extract_audio(&video_path, &output_audio_path)?;
    let au_cfg = get_audio_config(&output_audio_path)?;
    println!("got audio config : {:?}", au_cfg);

    let model = Arc::new(Model::new(&model_path).unwrap());
    let mut recognizer = Recognizer::new(&model, au_cfg.sample_rate as f32).unwrap();


    let _spec = WavSpec {
        sample_format: SampleFormat::Int,
        channels: au_cfg.channels,
        sample_rate: au_cfg.sample_rate,
        bits_per_sample: au_cfg.bits_per_sample,
    };

    let _ = process_audio_in_chunks(&output_audio_path, |audio| {
        let state = recognizer.accept_waveform(audio);
        match state {
            vosk::DecodingState::Finalized => {
                println!("Batch completed ");
                println!("{:?}", recognizer.final_result());
            }
            _ => {}
        }
        Ok(())
    })
    .unwrap();

    Ok(())
}
