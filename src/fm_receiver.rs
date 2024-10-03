use fsdr_blocks::type_converters::TypeConvertersBuilder;
use fsdr_blocks::futuresdr as futuresdr;
use futuresdr::blocks::VectorSink;
use futuresdr::runtime::Runtime;
use futuresdr::{
    blocks::{Apply, FirBuilder, VectorSinkBuilder, VectorSource},
    futuredsp::firdes,
    macros::connect,
    runtime::Flowgraph,
};
use hound::WavWriter;
use iqengine_plugin::server::{
    error::IQEngineError, CustomParamType, FunctionParameters, FunctionParamsBuilder,
    SamplesB64Builder,
};
use iqengine_plugin::server::{DataObject, FunctionOutput, FunctionRequest1, JobStatus};
use num_complex::Complex32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FmReceiverParams {
    #[serde(rename = "target_freq")]
    target_freq: f32,
}

pub struct FmReceiverFunction {}

impl iqengine_plugin::server::IQFunction1<FmReceiverParams> for FmReceiverFunction {
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

    async fn apply<I>(
        self,
        request: FunctionRequest1<FmReceiverParams>,
        job_status: JobStatus<I>,
    ) -> Result<FunctionOutput<I>, IQEngineError>
    where
        I: ToString + Send,
    {
        Result::Ok(FmReceiverFunction::apply(self, request, job_status).await?)
    }
}

impl FmReceiverFunction {
    async fn apply<I>(
        self,
        request: FunctionRequest1<FmReceiverParams>,
        job_status: JobStatus<I>,
    ) -> anyhow::Result<FunctionOutput<I>>
    where
        I: ToString + Send,
    {
        // debug!("Applying FM receiver...");

        // debug!("parameters checked.");
        let mut result = FunctionOutput::new(job_status);
        // let target_freq = request.custom_params.target_freq;
        // debug!("target_freq is {}", target_freq);
        let sample_rate = request.metadata.sample_rate.unwrap_or(1_800_000.0);
        // debug!("sample_rate is {}", sample_rate);
        match request.metadata.data_type {
            iqengine_plugin::server::DataType::IqSlashCf32Le => {
                let v = request.samples_cf32()?;

                let src = VectorSource::new(v);

                // TODO use fsdr-block shift
                // let mut last = Complex32::new(1.0, 0.0);
                // let add = Complex32::from_polar(
                //     1.0,
                //     (2.0 * std::f64::consts::PI * target_freq / (sample_rate as f64)) as f32,
                // );
                // let _shift = Apply::new(move |v: &Complex32| -> Complex32 {
                //     last *= add;
                //     last * v
                // });

                const AUDIO_RATE: f32 = 48_000.0;
                // Downsample to 480kHz before demodulation (will be, later on, decimated again)
                const INTERP: f32 = 10.0;
                const TARGET_RATE: f32 = AUDIO_RATE * INTERP;
                let decim = sample_rate / TARGET_RATE * INTERP;
                let interp = INTERP as usize;
                let decim = decim as usize;
                // debug!("resampling {}/{}", interp, decim);
                let resamp1 = FirBuilder::resampling::<Complex32, Complex32>(interp, decim);

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
                let resamp2 = FirBuilder::resampling_with_taps::<f32, f32, _>(
                    1,
                    INTERP as usize,
                    audio_filter_taps,
                );

                // Most audio players prefers int16
                let conv = TypeConvertersBuilder::lossy_scale_convert_f32_i16().build();

                let snk = VectorSinkBuilder::<i16>::new().build();
                // TODO later leverage let snk = WavSink::<i16>::new(file_name, spec);

                // Create the `Flowgraph` where the `Block`s will be added later on
                let mut fg = Flowgraph::new();
                // Add all the blocks to the `Flowgraph`...
                // connect!(fg, src > shift > resamp1 > demod > resamp2 > snk;);
                connect!(fg, src > resamp1 > demod > resamp2 > conv > snk;);

                // debug!("Starting FM receiver flow-graph");

                let fg = Runtime::new().run_async(fg).await?;

                let spec = hound::WavSpec {
                    channels: 1,
                    sample_rate: AUDIO_RATE as u32,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                };
                let mut wav_data = Vec::<u8>::new();
                let mut buf = std::io::Cursor::new(&mut wav_data);
                let mut writer = WavWriter::new(&mut buf, spec)?;
                let snk_0 = fg.kernel::<VectorSink<i16>>(snk).unwrap();
                let snk_0 = snk_0.items();
                let mut wav_writer = writer.get_i16_writer(snk_0.len() as u32);
                for audio_sample in snk_0 {
                    wav_writer.write_sample(*audio_sample);
                }
                wav_writer.flush()?;
                writer.finalize()?;

                let output = SamplesB64Builder::new().from_wav_data(wav_data).build()?;
                let data_object = DataObject {
                    data_type: Some(output.data_type),
                    file_name: Some("output.wav".to_string()),
                    data: output.samples,
                };
                result.non_iq_output_data = Some(data_object);
            }
            _ => {
                return anyhow::Result::Err(IQEngineError::UnsupportedDataType(
                    request.metadata.data_type,
                ).into());
            }
        }
        Ok(result)
    }
}

pub const FM_RECEIVER_FUNCTION: FmReceiverFunction = FmReceiverFunction {};
