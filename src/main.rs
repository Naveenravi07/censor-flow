use anyhow::{Context, Result};
use hound::{SampleFormat, WavReader, WavWriter};
use std::collections::VecDeque;
use std::f32::consts::PI;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use vosk::{Model, Recognizer};

fn get_badword_list(filepath: &PathBuf) -> Result<Vec<String>> {
    let file = fs::read_to_string(filepath)?;
    Ok(file.lines().map(|x| x.to_string()).collect())
}

fn extract_audio(video_path: &str, audio_output: &str) -> Result<(), std::io::Error> {
    println!("Extracting audio from video: {}", video_path);
    let output = Command::new("ffmpeg")
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

    if !output.status.success() {
        println!("FFmpeg stderr: {}", String::from_utf8_lossy(&output.stderr));
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "FFmpeg command failed",
        ));
    }
    println!("Audio extracted successfully");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        eprintln!(
            "Usage: {} <input_video> <extracted_audio> <output_censored_audio>",
            args[0]
        );
        std::process::exit(1);
    }

    let modal_path = &args[1];
    let input_video = &args[2];
    let extracted_audio = &args[3];
    let output_censored_audio = &args[4];

    extract_audio(input_video, extracted_audio)?;

    let bwordlst = get_badword_list(&PathBuf::from("./lib/badwordslist.txt"))?;
    println!("Loaded {} bad words", bwordlst.len());

    let model = Model::new(modal_path).context("Failed to load Vosk model")?;
    println!("Loaded Vosk model");

    let mut recognizer = Recognizer::new_with_grammar(&model, 16000.0, &bwordlst).unwrap();
    let bad_times: Arc<Mutex<VecDeque<(f32, f32)>>> = Arc::new(Mutex::new(VecDeque::new()));

    let file = File::open(extracted_audio).context("Failed to open extracted audio file")?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0; 4096];
    let mut total_samples = 0;
    let mut chunks_processed = 0;

    println!("Processing audio...");
    while let Ok(n) = reader.read(&mut buffer) {
        if n == 0 {
            break;
        }
        let samples: Vec<i16> = buffer[..n]
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        total_samples += samples.len();
        chunks_processed += 1;

        if let vosk::DecodingState::Finalized = recognizer.accept_waveform(&samples) {
            let result = recognizer.final_result().single().unwrap();
            let timelines: Vec<(f32, f32)> = result
                .result
                .iter()
                .filter_map(|w| {
                    if w.conf > 0.9 {
                        Some((w.start, w.end))
                    } else {
                        None
                    }
                })
                .collect();
            println!("\nDetected bad words: {:?}", result);
            println!("Timeline : {:?}", timelines);
            bad_times.lock().await.extend(timelines);
        }

        if chunks_processed % 100 == 0 {
            println!(
                "Processed {} chunks ({} samples)",
                chunks_processed, total_samples
            );
        }
    }

    println!(
        "Finished processing. Total samples: {}, Chunks processed: {}",
        total_samples, chunks_processed
    );

    let censored_times = bad_times.lock().await.drain(..).collect::<Vec<_>>();
    println!("Detected {} censored segments", censored_times.len());

    censor_audio_with_beep(extracted_audio, output_censored_audio, &censored_times)?;

    println!(
        "Finished censoring. Output written to: {}",
        output_censored_audio
    );

    Ok(())
}

fn generate_beep(sample_rate: u32, duration: f32, frequency: f32) -> Vec<i16> {
    let num_samples = (duration * sample_rate as f32) as usize;
    (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (i16::MAX as f32 * (2.0 * PI * frequency * t).sin()) as i16
        })
        .collect()
}

fn censor_audio_with_beep(
    input_file: &str,
    output_file: &str,
    censor_times: &[(f32, f32)],
) -> Result<()> {
    let mut reader = WavReader::open(input_file)?;
    let spec = reader.spec();
    let mut writer = WavWriter::create(output_file, spec)?;

    let sample_rate = spec.sample_rate;
    let mut censor_samples = censor_times
        .iter()
        .map(|&(start, end)| {
            (
                (start * sample_rate as f32).round() as usize,
                (end * sample_rate as f32).round() as usize,
            )
        })
        .collect::<Vec<_>>();
    censor_samples.sort_unstable_by_key(|&(start, _)| start);

    let beep_frequency = 1000.0;
    let beep = generate_beep(sample_rate, 0.5, beep_frequency);

    let mut current_censor = 0;
    let mut beep_index = 0;

    for (i, sample) in reader.samples::<i16>().enumerate() {
        let sample = sample?;
        if current_censor < censor_samples.len() && i >= censor_samples[current_censor].0 {
            if i < censor_samples[current_censor].1 {
                writer.write_sample(beep[beep_index])?;
                beep_index = (beep_index + 1) % beep.len();
            } else {
                writer.write_sample(sample)?;
                current_censor += 1;
                beep_index = 0;
            }
        } else {
            writer.write_sample(sample)?;
        }
    }

    Ok(())
}

