extern crate aubio;
extern crate cpal;

use std::io::{self, Write};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use aubio::tempo::Tempo;
use cpal::{Device, Format, EventLoop, UnknownTypeInputBuffer, StreamData, Sample};

fn input_stream(device: &Device, format: &Format)
    -> Result<Receiver<Vec<f32>>, cpal::CreationError>
{
    let (tx, rx) = mpsc::sync_channel(0);

    let event_loop = EventLoop::new();

    let stream = event_loop.build_input_stream(device, format)?;

    event_loop.play_stream(stream);

    thread::spawn(move || {
        event_loop.run(|_, data| {
            let samples = match data {
                StreamData::Input { buffer: UnknownTypeInputBuffer::U16(buffer) } =>
                    buffer.iter().map(Sample::to_f32).collect(),
                StreamData::Input { buffer: UnknownTypeInputBuffer::I16(buffer) } =>
                    buffer.iter().map(Sample::to_f32).collect(),
                StreamData::Input { buffer: UnknownTypeInputBuffer::F32(buffer) } =>
                    buffer.iter().map(Sample::to_f32).collect(),

                StreamData::Output { .. } => unreachable!()
            };

            tx.send(samples).expect("tx.send");
        });
    });

    Ok(rx)
}

#[derive(Debug)]
enum RunError {
    StreamCreate(cpal::CreationError),
    Aubio,
    Io(io::Error),
}

fn adjust_tempo(mut tempo: f32, lo: f32, hi: f32) -> f32 {
    while tempo < lo {
        tempo *= 2.0;
    }

    while tempo > hi {
        tempo /= 2.0;
    }

    tempo
}

fn run_tempo(device: &Device, format: &Format) -> Result<(), RunError> {
    const BUFFER_SIZE: usize = 1024;

    let stream = input_stream(device, format)
        .map_err(RunError::StreamCreate)?;

    let mut tempo = Tempo::new(BUFFER_SIZE, BUFFER_SIZE, format.sample_rate.0 as usize)
        .map_err(|()| RunError::Aubio)?;

    let mut buffer = Vec::new();

    for samples in stream {
        buffer.extend(samples);

        while buffer.len() > BUFFER_SIZE {
            tempo.execute(&buffer[0..BUFFER_SIZE]);
            buffer.drain(0..BUFFER_SIZE);
        }

        let mut stdout = io::stdout();

        match tempo.bpm() {
            Some(tempo) => write!(stdout, "\r\x1b[0K{} bpm ", adjust_tempo(tempo, 100.0, 215.0)),
            None => write!(stdout, "\r\x1b[0Klocking on... "),
        }.map_err(RunError::Io)?;

        stdout.flush().map_err(RunError::Io)?;
    }

    Ok(())
}

fn main() {
    let device = cpal::default_input_device()
        .expect("cpal::default_input_device");

    let format = device.default_input_format()
        .expect("device.default_input_format");

    println!("Listening on {} ({:?})", device.name(), format);
    println!();

    run_tempo(&device, &format)
        .expect("run_tempo");
}
