use std::sync::mpsc::{Sender, Receiver, channel};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, FromSample, Host, I24, Sample, SizedSample, Stream, SupportedStreamConfig
};
use filter::LowPassFilter;

mod filter;

pub struct Audio {
    device: Device,
    config: SupportedStreamConfig,
}

pub struct AudioInterface {
    pub tx: Sender<Sound>,
    stream: Stream,
}

struct OutputFilter {
    low_pass: filter::LowPassFilter
}

#[derive(Clone, PartialEq)]
pub enum Sound {
    None,
    SquareWave {
        period: f32,
        duty: f32,
        volume: f32
    },
    TriangleWave {
        period: f32,
    }
}

fn gen_sound(sound: &Sound, sample: f32, sample_rate: f32) -> (f32, f32) {
    match *sound {
        Sound::None => (0.0, 0.0),
        Sound::SquareWave { period, duty , volume} => {
            square_wave(period, duty, volume, sample, sample_rate)
        }
        Sound::TriangleWave { period} => {
            triangle_wave(period, sample, sample_rate)
        }
    }
}

fn square_wave(period: f32, duty: f32, volume: f32, sample: f32, sample_rate: f32) -> (f32, f32) {
    let t = (sample / sample_rate) % period;
    let volume = 0.1128 * volume;
    if t < duty * period {
        (sample + 1.0, volume)
    } else if t < period {
        (sample + 1.0, -volume)
    } else {
        (0.0, volume)
    }
}

fn triangle_wave(period: f32, sample: f32, sample_rate: f32) -> (f32, f32) {
    let t: f32 = (sample / sample_rate) % period;
    let volume = 0.12765;
    if t < period / 2.0 {
        (sample + 1.0, (1.0 - t * 4.0 / period) * volume)
    } else if t < period {
        (sample + 1.0, (t * 4.0 / period - 3.0) * volume)
    } else {
        (0.0, volume)
    }
}

impl OutputFilter {
    fn new(sample_period: f32) -> Self {
        Self {
            low_pass: LowPassFilter::new(sample_period, 14e3) // 14 kHz
        }
    }

    fn apply(&mut self, x: f32) -> f32 {
        let y = self.low_pass.apply(x);
        y
    }
}

impl Audio {
    pub fn new(audio_device: Option<String>) -> anyhow::Result<Self> {
        let host: cpal::Host = cpal::default_host();

        println!("Audio devices:");

        let device;
        if let Some(device_id) = audio_device {
            device = host.device_by_id(&device_id.parse()
                .expect("Could not parse audio device ID"))
                .expect("Could not find audio device ID");
        } else {
            device = host.default_output_device()
                .expect("failed to find output device");
            println!("Using default device: {}", device.id()?);
            println!("Other audio devices available:");
            for device in host.devices().unwrap() {
                let id = device.id().unwrap();
                println!("{}", id.to_string());
            }
        }

        let config = device.default_output_config().unwrap();
        println!("Default output config: {config:?}");

        Ok(Audio {
            device: device,
            config: config.into(),
        })
    }

    pub fn create_interface(&self) -> anyhow::Result<AudioInterface>  {
        let (tx, rx) = channel();
        let sample_rate = self.config.sample_rate() as f32;
        let filter = OutputFilter::new(sample_rate);
        let stream = match self.config.sample_format() {
            cpal::SampleFormat::I8 => self.run::<i8>(rx, filter),
            cpal::SampleFormat::I16 => self.run::<i16>(rx, filter),
            cpal::SampleFormat::I24 => self.run::<I24>(rx, filter),
            cpal::SampleFormat::I32 => self.run::<i32>(rx, filter),
            // cpal::SampleFormat::I48 => self.run::<I48>(rx, filter),
            cpal::SampleFormat::I64 => self.run::<i64>(rx, filter),
            cpal::SampleFormat::U8 => self.run::<u8>(rx, filter),
            cpal::SampleFormat::U16 => self.run::<u16>(rx, filter),
            // cpal::SampleFormat::U24 => self.run::<U24>(rx, filter),
            cpal::SampleFormat::U32 => self.run::<u32>(rx, filter),
            // cpal::SampleFormat::U48 => self.run::<U48>(rx, filter),
            cpal::SampleFormat::U64 => self.run::<u64>(rx, filter),
            cpal::SampleFormat::F32 => self.run::<f32>(rx, filter),
            cpal::SampleFormat::F64 => self.run::<f64>(rx, filter),
            sample_format => panic!("Unsupported sample format '{sample_format}'"),
        }?;

        Ok(AudioInterface {
            tx: tx,
            stream: stream,
        })
    }

    fn run<T>(&self, rx: Receiver<Sound>, mut filter: OutputFilter) -> Result<Stream, anyhow::Error>
    where
        T: SizedSample + FromSample<f32>,
    {
        let config = self.config.config();
        let sample_rate = config.sample_rate as f32;
        let channels = config.channels as usize;

        // Generate sound
        let mut wave_sample = 0.0;
        // let mut duration_s = LENGTH_TICK;
        let mut sound = Sound::None;
        let mut next_value = move || {
            if let Ok(new_sound) = rx.try_recv() {
                if new_sound != sound {
                    sound = new_sound;
                    wave_sample = 0.0;
                }
            }
            let (next_sample, val) = gen_sound(&sound, wave_sample, sample_rate);
            let val = filter.apply(val);
            wave_sample = next_sample;
            val
        };

        let err_fn = |err| eprintln!("an error occurred on stream: {err}");

        let stream = self.device.build_output_stream(
            &config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                write_data(data, channels, &mut next_value)
            },
            err_fn,
            None,
        )?;

        stream.play()?;

        Ok(stream)
    }
}

fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut dyn FnMut() -> f32)
where
    T: Sample + FromSample<f32>,
{
    for frame in output.chunks_mut(channels) {
        let value: T = T::from_sample(next_sample());
        for sample in frame.iter_mut() {
            *sample = value;
        }
    }
}