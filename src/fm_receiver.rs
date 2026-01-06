use fsdr_blocks::type_converters::TypeConvertersBuilder;
use futuresdr::blocks::VectorSink;
use futuresdr::runtime::Runtime;
use futuresdr::{
    blocks::{Apply, FirBuilder, VectorSinkBuilder, VectorSource},
    futuredsp::firdes,
    log::debug,
    macros::connect,
    runtime::Flowgraph,
};
use base64::Engine;
use hound::WavWriter;
use iqengine_plugin::server::{
    error::IQEngineError, CustomParamType, FunctionParameters, FunctionParamsBuilder,
    FunctionPostRequest,
};
use num_complex::Complex32;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FmReceiverParams {
    #[serde(rename = "target_freq")]
    target_freq: f32,
}

pub struct FmReceiverFunction {}

impl iqengine_plugin::server::IQFunction<FmReceiverParams> for FmReceiverFunction {
    fn parameters(&self) -> FunctionParameters {
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
        &self,
        request: FunctionPostRequest<FmReceiverParams>,
        samples: Vec<num_complex::Complex32>,
        job_id: String,
        job_store: std::sync::Arc<iqengine_plugin::server::JobStore>,
    ) -> Result<iqengine_plugin::server::Output, IQEngineError> {
        debug!("Applying FM receiver for job {}...", job_id);
        
        debug!("parameters checked.");
        
        let target_freq = if let Some(prop) = request.custom_params {
            prop.target_freq as f64
        } else {
            return Err(IQEngineError::MandatoryParameter("target_freq".to_string()));
        };
        debug!("target_freq is {}", target_freq);
        
        let sample_rate = request.metadata_file.sample_rate as f64;
        debug!("sample_rate is {}", sample_rate);

        let src = VectorSource::new(samples);

        // ... existing flowgraph logic ...
        const AUDIO_RATE: f32 = 48_000.0;
        const INTERP: f32 = 10.0;
        const TARGET_RATE: f32 = AUDIO_RATE * INTERP;
        let decim = sample_rate / TARGET_RATE as f64 * INTERP as f64;
        let interp = INTERP as usize;
        let decim = decim as usize;
        let resamp1 = FirBuilder::new_resampling::<Complex32, Complex32>(interp, decim);

        let mut last = Complex32::new(0.0, 0.0);
        let demod = Apply::new(move |v: &Complex32| -> f32 {
            let arg = (v * last.conj()).arg();
            last = *v;
            arg
        });

        let cutoff = 2_000.0 / AUDIO_RATE as f64;
        let transition = 10_000.0 / AUDIO_RATE as f64;
        let audio_filter_taps = firdes::kaiser::lowpass::<f32>(cutoff, transition, 0.1);
        let resamp2 = FirBuilder::new_resampling_with_taps::<f32, f32, _, _>(
            1,
            INTERP as usize,
            audio_filter_taps,
        );

        let conv = TypeConvertersBuilder::lossy_scale_convert_f32_i16().build();
        let snk = VectorSinkBuilder::<i16>::new().build();

        let mut fg = Flowgraph::new();
        connect!(fg, src > resamp1 > demod > resamp2 > conv > snk;);

        debug!("Starting FM receiver flow-graph");
        
        // Report progress
        if let Ok(mut status) = job_store.get_job_status(&job_id) {
            status.progress = 50.0;
            let _ = job_store.save_job_status(&status);
        }

        let fg = Runtime::new().run_async(fg).await.map_err(|e| IQEngineError::GenericError(e.to_string()))?;

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: AUDIO_RATE as u32,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut wav_data = Vec::<u8>::new();
        let mut buf = std::io::Cursor::new(&mut wav_data);
        let mut writer = WavWriter::new(&mut buf, spec).map_err(IQEngineError::from)?;
        let snk_0 = fg.kernel::<VectorSink<i16>>(snk).unwrap();
        let snk_0 = snk_0.items();
        for audio_sample in snk_0 {
            writer.write_sample(*audio_sample).map_err(IQEngineError::from)?;
        }
        writer.finalize().map_err(IQEngineError::from)?;

        let base64_wav = base64::engine::general_purpose::STANDARD.encode(wav_data);
        
        let mut output = iqengine_plugin::server::Output::new();
        output.data_output = Some(vec![iqengine_plugin::server::DataObject::new(
            iqengine_plugin::server::DataType::AudioSlashWav,
            format!("{}.wav", request.metadata_file.file_name),
            base64_wav,
        )]);
        
        Ok(output)
    }
}

pub const FM_RECEIVER_FUNCTION: FmReceiverFunction = FmReceiverFunction {};
