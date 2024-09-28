use iqengine_plugin::server::{
    error::IQEngineError, CustomParamType, FunctionOutput, FunctionParameters,
    FunctionParamsBuilder, JobStatus, SamplesB64Builder,
};
use iqengine_plugin::server::{DataObject, FunctionRequest1, Metadata};

use num_complex::Complex32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmplifierParams {
    #[serde(rename = "a")]
    a: f32,
}

pub struct AmplifierFunction {}

impl iqengine_plugin::server::IQFunction1<AmplifierParams> for AmplifierFunction {
    fn parameters(self) -> FunctionParameters {
        FunctionParamsBuilder::new()
            .max_inputs(1)
            .max_outputs(1)
            .custom_param(
                "a",
                "amplitude multiplier coefficient",
                CustomParamType::Number,
                Some("1.0"),
            )
            .build()
    }

    async fn apply<I>(
        self,
        request: FunctionRequest1<AmplifierParams>,
        job_status: JobStatus<I>,
    ) -> Result<FunctionOutput<I>, IQEngineError>
    where
        I: ToString + Send,
    {
        let mut result = FunctionOutput::new(job_status);
        let a = request.custom_params.a;
        match request.metadata.data_type {
            iqengine_plugin::server::DataType::IqSlashCf32Le => {
                result.metadata_file = Some(request.metadata.clone());
                // result.metadata_file = Some(Metadata {
                //     file_name: request.metadata.file_name,
                //     sample_rate: request.metadata.sample_rate,
                //     center_freq: request.metadata.center_freq,
                //     data_type: request.metadata.data_type,
                // });
                let v = request.samples_cf32()?;
                let o = v.iter().map(|iq| iq * a);
                let o: Vec<Complex32> = o.collect();
                let output = SamplesB64Builder::new().with_samples_cf32(o).build()?;

                result.output_data = Some(output.samples);
            }
            _ => {
                return Err(IQEngineError::UnsupportedDataType(
                    request.metadata.data_type,
                ));
            }
        }
        Ok(result)
    }
}

pub const AMPLIFIER_FUNCTION: AmplifierFunction = AmplifierFunction {};
