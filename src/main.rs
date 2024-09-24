use anyhow::Result;
use cpal::{
    self,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use std::{
    env, i16,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};
use vosk::{Model, Recognizer};
fn main() -> Result<()> {
    let mut args = env::args();
    args.next();
    let model_path = args.next().expect("Model path not found");
    let host = cpal::default_host();
    let default_in = host
        .default_input_device()
        .expect("No default input device available");
    let default_out = host
        .default_output_device()
        .expect("No default output device available");
    println!("Default input device: {:?}", default_in.name()?);
    println!("Default output device: {:?}", default_out.name()?);
    let mut cfg_in = default_in.supported_input_configs()?;
    let supported_in = cfg_in
        .next()
        .expect("No supported input config")
        .with_max_sample_rate();
    println!("Max supported input: {:?}", supported_in);
    let mut cfg_out = default_out.supported_output_configs()?;
    let supported_out = cfg_out
        .next()
        .expect("No supported output config")
        .with_max_sample_rate();
    println!("Max supported output: {:?}", supported_out);
    let model = Arc::new(Model::new(&model_path).unwrap());
    let recognizer = Arc::new(Mutex::new(
        Recognizer::new(&model, supported_in.sample_rate().0 as f32).unwrap(),
    ));
    let (tx, rx) = mpsc::channel::<Vec<i16>>();
    let tx_clone = tx.clone();
    let stream = default_in.build_input_stream(
        &supported_in.config(),
        move |data: &[i16], _: &cpal::InputCallbackInfo| {
            // Send the captured audio data to the other thread
            if let Err(err) = tx_clone.send(data.to_vec()) {
                eprintln!("Error sending audio data: {:?}", err);
            }
        },
        on_err,
        None,
    )?;
    stream.play()?;
    let recognizer_clone = Arc::clone(&recognizer);
    thread::spawn(move || {
        while let Ok(audio_data) = rx.recv() {
            let mut reco = recognizer_clone.lock().unwrap();
            reco.accept_waveform(&audio_data);
            println!("Partial Result: {:?}", reco.partial_result());
            let out = default_out.build_output_stream(
                &supported_out.config().into(),
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    for (out_sample, in_sample) in data.iter_mut().zip(audio_data.iter()) {
                        *out_sample = *in_sample;
                    }
                },
                on_err,
                None,
            );
            out.unwrap().play().unwrap();
        }
    });

    thread::sleep(Duration::from_secs(10));
    let mut final_result = recognizer.lock().unwrap();
    println!("Final Result: {:?}", final_result.result());
    Ok(())
}
fn on_err(err: cpal::StreamError) {
    eprintln!("An error occurred in the input stream: {:?}", err);
}
