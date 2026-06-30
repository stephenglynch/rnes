use std::sync::mpsc::{Receiver, Sender, channel};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, FromSample, I24, Sample, SizedSample, Stream, SupportedStreamConfig
};
use filter::LowPassFilter;

mod filter;

pub struct NesSample {
    pub volume: f32,
    pub duration: f32
}

pub struct NesStream {
    current: Option<NesSample>,
    stream: Receiver<NesSample>
}

impl NesStream {
    fn new(rx: Receiver<NesSample>) -> Self {
        NesStream { current: None, stream: rx }
    }

    fn next_sample(&mut self, sample_time: f32) -> Option<f32> {
        let mut sample_time_left = 0.0;
        if let Some(nes_sample) = &mut self.current {
            nes_sample.duration -= sample_time;
            if nes_sample.duration >= 0.0 {
                return Some(nes_sample.volume);
            } else {
                // Not enough time remaining in current sample discard, and use
                // remaining time into the next sample
                sample_time_left = -nes_sample.duration;
            }
        } else {
            if let Ok(nes_sample) = self.stream.try_recv() {
                self.current = Some(nes_sample);
                return self.next_sample(sample_time);
            }
        }
        // Recurse into the next sample if the current sample is not long enough
        if sample_time_left > 0.0 {
            self.current = self.stream.try_recv().ok();
            return self.next_sample(sample_time_left);
        }
        None
    }
}

type Sound = NesSample;

pub struct Audio {
    device: Device,
    config: SupportedStreamConfig,
}

pub struct AudioInterface {
    pub tx: Sender<Sound>,
    _stream: Stream,
}

struct OutputFilter {
    low_pass: filter::LowPassFilter
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

    pub fn create_interface(&self, id: usize) -> anyhow::Result<AudioInterface>  {
        let (tx, rx) = channel();
        let sample_rate = self.config.sample_rate() as f32;
        let filter = OutputFilter::new(sample_rate);
        let stream = match self.config.sample_format() {
            cpal::SampleFormat::I8 => self.run::<i8>(id, rx, filter),
            cpal::SampleFormat::I16 => self.run::<i16>(id, rx, filter),
            cpal::SampleFormat::I24 => self.run::<I24>(id, rx, filter),
            cpal::SampleFormat::I32 => self.run::<i32>(id, rx, filter),
            // cpal::SampleFormat::I48 => self.run::<I48>(id, rx, filter),
            cpal::SampleFormat::I64 => self.run::<i64>(id, rx, filter),
            cpal::SampleFormat::U8 => self.run::<u8>(id, rx, filter),
            cpal::SampleFormat::U16 => self.run::<u16>(id, rx, filter),
            // cpal::SampleFormat::U24 => self.run::<U24>(id, rx, filter),
            cpal::SampleFormat::U32 => self.run::<u32>(id, rx, filter),
            // cpal::SampleFormat::U48 => self.run::<U48>(id, rx, filter),
            cpal::SampleFormat::U64 => self.run::<u64>(id, rx, filter),
            cpal::SampleFormat::F32 => self.run::<f32>(id, rx, filter),
            cpal::SampleFormat::F64 => self.run::<f64>(id, rx, filter),
            sample_format => panic!("Unsupported sample format '{sample_format}'"),
        }?;

        Ok(AudioInterface {
            tx: tx,
            _stream: stream,
        })
    }

    fn run<T>(&self, _id: usize, rx: Receiver<Sound>, mut filter: OutputFilter) -> Result<Stream, anyhow::Error>
    where
        T: SizedSample + FromSample<f32>,
    {
        let config = self.config.config();
        let sample_rate = config.sample_rate as f32;
        let channels = config.channels as usize;

        println!("sample_rate = {}", sample_rate);
        let sample_time = 1.0 / sample_rate;

        let mut nes_stream = NesStream::new(rx);
        let mut next_value = move || {
            let sample = nes_stream.next_sample(sample_time).unwrap_or_default();
            filter.apply(sample)
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