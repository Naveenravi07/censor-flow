use anyhow::Result;
use cpal::{
    traits::{HostTrait, StreamTrait},
    HostId,
};
use rodio::DeviceTrait;
use std::{
    env,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};
use vosk::{Model, Recognizer};

fn main() -> Result<()> {
    let mut args = env::args();
    args.next();
    let model_path = args.next().expect("Model path not found");
    let host = cpal::host_from_id(HostId::Alsa).expect("Failed to unwrap host");

    let default_in = host
        .default_input_device()
        .expect("No default input device available");
    let default_out = host.default_output_device().unwrap();

    let mut cfg_in = default_in.supported_input_configs()?;
    let supported_in = cfg_in
        .next()
        .expect("No supported input config")
        .with_max_sample_rate();
    let mut cfg_out = default_out.supported_output_configs()?;
    let supported_out = cfg_out
        .next()
        .expect("No supported output config")
        .with_max_sample_rate();

    let model = Arc::new(Model::new(&model_path).unwrap());
    let recognizer = Arc::new(Mutex::new(
        Recognizer::new(&model, supported_in.sample_rate().0 as f32).unwrap(),
    ));

    let (tx, rx) = mpsc::channel::<Vec<i16>>();
    let shared_audio_data = Arc::new(Mutex::new(Vec::new()));

    // Build input stream
    let tx_clone = tx.clone();
    let shared_audio_data_in = Arc::clone(&shared_audio_data);
    let input_stream = default_in.build_input_stream(
        &supported_in.config(),
        move |data: &[i16], _: &cpal::InputCallbackInfo| {
            if let Err(err) = tx_clone.send(data.to_vec()) {
                eprintln!("Error sending audio data: {:?}", err);
            }
            // Update the shared audio data
            let mut audio = shared_audio_data_in.lock().unwrap();
            *audio = data.to_vec();
        },
        on_err,
        None,
    )?;
    input_stream.play()?;

    // Build output stream
    let shared_audio_data_out = Arc::clone(&shared_audio_data);
    let output_stream = default_out.build_output_stream(
        &supported_out.config(),
        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
            let audio_data = shared_audio_data_out.lock().unwrap();
            for (out_sample, in_sample) in data.iter_mut().zip(audio_data.iter()) {
                *out_sample = *in_sample;
            }
        },
        on_err,
        None,
    )?;
    output_stream.play()?;

    let recognizer_clone = Arc::clone(&recognizer);
    thread::spawn(move || {
        while let Ok(audio_data) = rx.recv() {
            let mut reco = recognizer_clone.lock().unwrap();
            reco.accept_waveform(&audio_data);
            println!("Partial Result: {:?}", reco.partial_result());
        }
    });

    thread::sleep(Duration::from_secs(10));

    let mut final_result = recognizer.lock().unwrap();
    println!("Final Result: {:?}", final_result.result());
    Ok(())
}

fn on_err(err: cpal::StreamError) {
    eprintln!("An error occurred in the input/output stream: {:?}", err);
}
