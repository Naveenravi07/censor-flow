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

    // Build input stream
    let tx_clone = tx.clone();
    let input_stream = default_in.build_input_stream(
        &supported_in.config(),
        move |data: &[i16], _: &cpal::InputCallbackInfo| {
            tx_clone.send(data.to_vec()).unwrap();
        },
        on_err,
        None,
    )?;
    input_stream.play()?;

    //Building output stream
    let output_stream = default_out.build_output_stream(
        &supported_out.config(),
        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
            if let Ok(audio_data) = rx.recv() {
                for (out_sample, in_sample) in data.iter_mut().zip(audio_data.iter()) {
                    *out_sample = *in_sample;
                }
            } else {
                eprintln!("Receive error ");
            }
        },
        on_err,
        None,
    )?;
    output_stream.play()?;


    thread::sleep(Duration::from_secs(10));
    let mut final_result = recognizer.lock().unwrap();
    println!("Final Result: {:?}", final_result.result());
    Ok(())
}

fn on_err(err: cpal::StreamError) {
    eprintln!("An error occurred in the input/output stream: {:?}", err);
}
