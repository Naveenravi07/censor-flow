use aho_corasick::AhoCorasick;
use anyhow::Result;
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::io::BufWriter;
use std::{
    env,
    fs::{self},
    path::PathBuf,
    process::Command,
    sync::Arc,
};
use vosk::{Model, Recognizer};

const BUFFER_LEN: usize = 16000;

#[derive(Debug, Clone)]
struct AudioConfig {
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
}

#[derive(Debug, Clone)]
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

fn process_audio_in_chunks<F>(file_path: &str, overflow_rate: u16, mut cb: F) -> Result<()>
where
    F: FnMut(&Vec<i16>) -> Result<()>,
{
    let mut buff: Vec<i16> = Vec::with_capacity(BUFFER_LEN);
    let mut overflow_buf: Vec<i16> = Vec::with_capacity(overflow_rate as usize);
    let mut reader = WavReader::open(file_path)?;

    for sample in reader.samples::<i16>() {
        let sample = sample?;

        if buff.len() == 0 {
            buff = overflow_buf.clone();
            overflow_buf.clear();
        }

        buff.push(sample);

        if buff.len() == BUFFER_LEN {
            let _ = cb(&buff);
            let (_, right) = buff.split_at(BUFFER_LEN - (overflow_rate as usize));
            overflow_buf = right.to_vec();
            buff.clear();
        }
    }

    if !buff.is_empty() {
        let _ = cb(&buff);
    }

    Ok(())
}

fn beep_bad_words<F>(
    mut bads: Vec<Bword>,
    mut audio: Vec<i16>,
    au_cfg: &AudioConfig,
    mut cb: F,
) -> Result<()>
where
    F: FnMut(&Vec<i16>) -> Result<()>,
{
    for bword in &bads {
        let start_sample = (bword.start as u32 * au_cfg.sample_rate) / 1000;
        let end_sample = (bword.end as u32 * au_cfg.sample_rate) / 1000;

        println!(
            "Beeping between samples {} and {}",
            start_sample, end_sample
        );

        for i in start_sample..end_sample {
            if i < audio.len() as u32 {
                audio[i as usize] = audio[i as usize];
            } else {
                println!("Warning: Out of bounds access at sample {}", i);
            }
        }
    }
    let _ = cb(&audio);
    bads.clear();
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let start_time = std::time::Instant::now();

    let mut args = env::args();
    args.next();
    let model_path = args.next().expect("Model path not found");
    let video_path = args.next().expect("Video path not found");
    let output_audio_path = args.next().expect("Output path not found");
    let censored_audio_path = args.next().expect("Censored Output path not found");

    extract_audio(&video_path, &output_audio_path)?;
    let au_cfg = get_audio_config(&output_audio_path)?;
    println!("got audio config : {:?}", au_cfg);

    let model = Arc::new(Model::new(&model_path).unwrap());
    let mut recognizer = Recognizer::new(&model, au_cfg.sample_rate as f32).unwrap();
    recognizer.set_words(true);

    let bwordlst = get_badword_list(&PathBuf::from("./lib/badwordslist.txt"))?;
    let ac = AhoCorasick::new(&bwordlst)?;

    let spec = WavSpec {
        sample_format: SampleFormat::Int,
        channels: au_cfg.channels,
        sample_rate: au_cfg.sample_rate,
        bits_per_sample: au_cfg.bits_per_sample,
    };
    let file = std::fs::File::create(&censored_audio_path).unwrap();
    let buf_writer = BufWriter::new(file);
    let mut wav_writer = WavWriter::new(buf_writer, spec).unwrap();
    let mut idx = 0;

    let _ = process_audio_in_chunks(&output_audio_path, 800, |audio| {
        let state = recognizer.accept_waveform(&audio);

        match state {
            vosk::DecodingState::Finalized => {
                idx += 1;
                println!("\n \n Batch completed ");

                let mut bad_words: Vec<Bword> = Vec::new();
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

                println!("Bad words detected: {:?}", bad_words);

                let fpp_org = PathBuf::from(&format!("/home/shastri/tmp/c_flow/org/{}.wav", idx));
                let mut wav_writer_org = WavWriter::create(&fpp_org, spec).unwrap();
                for &org in audio {
                    wav_writer_org.write_sample(org).unwrap();
                }
                wav_writer_org.flush().unwrap();

                beep_bad_words(bad_words, audio.to_vec(), &au_cfg, |censored_audio| {
                    println!(
                        "Writing chunk {}: original size = {}, censored size = {}",
                        idx,
                        audio.len(),
                        censored_audio.len()
                    );

                    let fpp = PathBuf::from(&format!("/home/shastri/tmp/c_flow/dbg/{}.wav", idx));
                    let mut wav_writer_dbg = WavWriter::create(&fpp, spec).unwrap();

                    for &sample in censored_audio {
                        wav_writer.write_sample(sample).unwrap();
                        wav_writer_dbg.write_sample(sample).unwrap();
                    }
                    wav_writer_dbg.flush().unwrap();
                    wav_writer.flush().unwrap();

                    Ok(())
                })
                .unwrap();
            }
            _ => {}
        }

        Ok(())
    })
    .unwrap();
    wav_writer.finalize().unwrap();

    println!("Process completed, Time Elapsed {:?}", start_time.elapsed());
    Ok(())
}
