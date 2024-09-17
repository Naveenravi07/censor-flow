use std::{env, sync::Arc};

use anyhow::Result;
use cpal::{
    self,
    traits::{DeviceTrait, HostTrait, StreamTrait},
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
    println!(" Default in : {:?}", default_in.name()?);
    println!(" Default out : {:?}", default_out.name()?);

    let mut cfg_in = default_in.supported_input_configs()?;
    let supported_in = cfg_in
        .next()
        .expect("No supported input config")
        .with_max_sample_rate();
    println!("Max supported in : {:?}", supported_in);

    let mut cfg_out = default_out.supported_output_configs()?;
    let supported_out = cfg_out
        .next()
        .expect("No supported output config")
        .with_max_sample_rate();
    println!("Max supported out : {:?}", supported_out);

    let model = Arc::new(Model::new(&model_path).unwrap());
    let mut recognizer = Recognizer::new(&model, supported_in.sample_rate().0 as f32).unwrap();

    let stream = default_in.build_input_stream(
        &supported_in.config(),
        move |data: &[i16], _: &cpal::InputCallbackInfo| {
            println!("Received input data with {} samples", data.len());
            recognizer.accept_waveform(&data);
            let result = recognizer.partial_result();
            println!("Result = {:?} ", result);
        },
        on_err,
        None,
    )?;

    stream.play()?;

    std::thread::sleep(std::time::Duration::from_secs(5));

    Ok(())
}

fn on_err(err: cpal::StreamError) {
    eprintln!("An error occurred in the input stream: {:?}", err);
}
