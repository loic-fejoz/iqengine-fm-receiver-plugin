use std::path::Path;

use fsdr_blocks::type_converters::TypeConvertersBuilder;
use futuresdr::{blocks::{VectorSinkBuilder, VectorSource, FirBuilder, Apply, audio::WavSink}, futuredsp::firdes, runtime::Flowgraph, macros::connect, log::debug};
use iqengine_plugin::server::{
    error::IQEngineError, Annotation, CustomParamType, FunctionParameters, FunctionParamsBuilder,
    FunctionPostRequest, FunctionPostResponse, SamplesB64Builder,
};
use num_complex::Complex32;
use futuresdr::async_io::block_on;
use futuresdr::runtime::Runtime;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FmReceiverParams {
    #[serde(rename = "target_freq")]
    target_freq: f32,
}

pub struct FmReceiverFunction {}

impl iqengine_plugin::server::IQFunction<FmReceiverParams> for FmReceiverFunction {
    fn parameters(self) -> FunctionParameters {
        FunctionParamsBuilder::new()
            .max_inputs(1)
            .max_outputs(1)
            .custom_param(
                "target_freq",
                "Center of FM carrier",
                CustomParamType::Number,
                Some("0.0"),
            )
            .build()
    }

    async fn apply(
        self,
        request: FunctionPostRequest<FmReceiverParams>,
    ) -> Result<FunctionPostResponse, IQEngineError> {
        debug!("Applying FM receiver...");
        if let Some(samples_cloud) = request.samples_cloud {
            if !samples_cloud.is_empty() {
                return Err(IQEngineError::NotYetImplemented(
                    "Cloud samples not yet implemented".to_string(),
                ));
            }
        }
        if request.samples_b64.is_none() {
            return Err(IQEngineError::NotYetImplemented(
                "samples in Base64 are mandatory".to_string(),
            ));
        }
        debug!("parameters checked.");
        let mut result = FunctionPostResponse::new();
        if let Some(samples_b64) = request.samples_b64 {
            let target_freq = if let Some(prop) = request.custom_params {
                prop.target_freq as f64
            } else {
                return Err(IQEngineError::MandatoryParameter("target_freq".to_string()));
            };
            debug!("target_freq is {}", target_freq);
            let stream1 = samples_b64.get(0).unwrap();
            let sample_rate = stream1.sample_rate.unwrap_or(1_800_000.0);
            debug!("sample_rate is {}", sample_rate);
            match stream1.data_type {
                iqengine_plugin::server::DataType::IqSlashCf32Le => {
                    let v = stream1.clone().samples_cf32()?;
                    
                    let src = VectorSource::new(v);

                    let mut last = Complex32::new(1.0, 0.0);
                    let add = Complex32::from_polar(
                        1.0,
                        (2.0 * std::f64::consts::PI * target_freq / (sample_rate as f64)) as f32,
                    );
                    let shift = Apply::new(move |v: &Complex32| -> Complex32 {
                        last *= add;
                        last * v
                    });

                    const AUDIO_RATE: f32 = 48_000.0;
                    // Downsample to 480kHz before demodulation (will be later on decimated again)
                    const INTERP: f32 = 10.0;
                    const TARGET_RATE: f32 = AUDIO_RATE * INTERP;
                    let decim = sample_rate / TARGET_RATE * INTERP;
                    let interp = INTERP as usize;
                    let decim = decim as usize;
                    debug!("resampling {}/{}", interp, decim);
                    let resamp1 = FirBuilder::new_resampling::<Complex32, Complex32>(interp, decim);

                    // Demodulation block using the conjugate delay method
                    // See https://en.wikipedia.org/wiki/Detector_(radio)#Quadrature_detector
                    let mut last = Complex32::new(0.0, 0.0); // store sample x[n-1]
                    let demod = Apply::new(move |v: &Complex32| -> f32 {
                        let arg = (v * last.conj()).arg(); // Obtain phase of x[n] * conj(x[n-1])
                        last = *v;
                        arg
                    });
                
                    // Design filter for the audio and decimate by INTERP.
                    // Ideally, this should be a FM de-emphasis filter, but the following works.
                    let cutoff = 2_000.0 / AUDIO_RATE as f64;
                    let transition = 10_000.0 / AUDIO_RATE as f64;
                    let audio_filter_taps = firdes::kaiser::lowpass::<f32>(cutoff, transition, 0.1);
                    let resamp2 = FirBuilder::new_resampling_with_taps::<f32, f32, _, _>(
                        1,
                        INTERP as usize,
                        audio_filter_taps,
                    );

                    // Most audio players prefers int16
                    let conv = TypeConvertersBuilder::lossy_scale_convert_f32_i16().build();

                    let filename = "/tmp/output.wav"; // TODO: not safe
                    let path = Path::new(filename);
                    let spec = hound::WavSpec {
                        channels: 1,
                        sample_rate: AUDIO_RATE as u32,
                        bits_per_sample: 16,
                        sample_format: hound::SampleFormat::Int,
                    };
                    let snk = WavSink::<i16>::new(path, spec);

                    // Create the `Flowgraph` where the `Block`s will be added later on
                    let mut fg = Flowgraph::new();
                    // Add all the blocks to the `Flowgraph`...
                    // connect!(fg, src > shift > resamp1 > demod > resamp2 > snk;);
                    connect!(fg, src > resamp1 > demod > resamp2 > conv > snk;);

                    debug!("Starting FM receiver flow-graph");
                    
                    //let _exec = Runtime::new().run(fg);
                    let _exec = Runtime::new().run_async(fg).await?;
                    //let _exec = block_on(Runtime::new().run_async(fg));
                    // if exec.is_err() {
                    //     return Err(IQEngineError::FutureSDRError(exec.unwrap_err()));
                    // }

                    let output = SamplesB64Builder::same_as(stream1)
                        .from_wav_file(path)
                        .build()?;
                    result.data_output = Some(vec![output]);
                }
                _ => {
                    return Err(IQEngineError::UnsupportedDataType(stream1.data_type));
                }
            }
        }
        Ok(result)
    }
}

pub const FM_RECEIVER_FUNCTION: FmReceiverFunction = FmReceiverFunction {};
