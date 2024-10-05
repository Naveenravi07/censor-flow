use aho_corasick::AhoCorasick;
use anyhow::Result;
use hound::{SampleFormat, WavReader, WavSpec};
use rodio::cpal::FromSample;
use std::{
    env, fs::{self}, path::PathBuf, process::Command, sync::Arc, thread, usize
};
use vosk::{Model, Recognizer};

const BUFFER_LEN: usize = 8192;

#[derive(Debug)]
struct AudioConfig {
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
}

#[derive(Debug,Clone)]
struct Bword {
    start: usize,
    end: usize,
    pattern: u32,
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

fn get_badword_list(filepath: &PathBuf) -> Result<Vec<String>> {
    let file = fs::read_to_string(&filepath).unwrap();
    let content = file.lines().map(|x| x.to_string()).collect();
    Ok(content)
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

fn process_audio_in_chunks<F>(file_path: &str, overfolw_rate: u16, mut cb: F) -> Result<()>
where
    F: FnMut(&Vec<i16>) -> Result<()>,
{
    let mut buff: Vec<i16> = Vec::with_capacity(BUFFER_LEN);
    let mut overfow_buf: Vec<i16> = Vec::with_capacity(overfolw_rate as usize);
    let mut reader = WavReader::open(file_path)?;

    for sample in reader.samples::<i16>() {
        let sample = sample?;

        if buff.len() == 0 {
            buff = overfow_buf.clone();
            overfow_buf.clear();
        }

        buff.push(sample);

        if buff.len() == BUFFER_LEN {
            let _ = cb(&buff);
            let (_, right) = buff.split_at(BUFFER_LEN - (overfolw_rate as usize));
            overfow_buf = right.to_vec();
            buff.clear();
        }
    }

    Ok(())
}

fn beep_bad_words(bads: Vec<Bword>, audio: Vec<i16>) -> Result<()> {
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let start_time = std::time::Instant::now();

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
    recognizer.set_words(true);

    let bwordlst = get_badword_list(&PathBuf::from("./lib/badwordslist.txt"))?;
    let ac = AhoCorasick::new(&bwordlst)?;

    let _spec = WavSpec {
        sample_format: SampleFormat::Int,
        channels: au_cfg.channels,
        sample_rate: au_cfg.sample_rate,
        bits_per_sample: au_cfg.bits_per_sample,
    };

    let _ = process_audio_in_chunks(&output_audio_path, 100, |audio| {
        let state = recognizer.accept_waveform(audio);
        let mut bad_words: Vec<Bword> = Vec::new();

        match state {
            vosk::DecodingState::Finalized => {
                println!("\n \n Batch completed ");

                let result = recognizer.final_result().single().unwrap();
                let haystack = result.text;
                println!("{}", haystack);

                for word in ac.find_iter(haystack) {
                    let bad_w = Bword {
                        start: word.start(),
                        end: word.end(),
                        pattern: word.pattern().as_u32(),
                    };
                    bad_words.push(bad_w);
                }

                println!("Badwords detected : {:?}", bad_words);

                let bad_words_clone = bad_words.clone();
                let audio_clone = audio.clone();

                thread::spawn(move || {
                    beep_bad_words(bad_words_clone, audio_clone).unwrap();
                });
            }
            _ => {}
        }

        Ok(())
    })
    .unwrap();

    println!("Process completed, Time Elapsed {:?}", start_time.elapsed());
    Ok(())
}

