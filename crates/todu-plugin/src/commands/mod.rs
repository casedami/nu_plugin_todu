mod add;
mod clear;
mod edit;
mod list;
mod rm;
mod status;

pub use add::ToduAdd;
pub use clear::ToduClear;
pub use edit::{ToduDesc, ToduDue, ToduTag};
pub use list::ToduList;
pub use rm::ToduRm;
pub use status::{ToduDone, ToduPause, ToduReopen, ToduStart, ToduStop};

use nu_plugin::EvaluatedCall;
use nu_protocol::{LabeledError, PipelineData};

pub(crate) fn collect_ids(
    call: &EvaluatedCall,
    input: PipelineData,
) -> Result<Vec<i64>, LabeledError> {
    match input {
        PipelineData::Empty => {
            let id: Option<i64> = call.opt(0)?;
            Ok(vec![id.ok_or_else(|| {
                LabeledError::new("provide an ID argument or pipe a list of IDs")
            })?])
        }
        _ => input
            .into_iter()
            .map(|v| v.as_int().map_err(|e| LabeledError::new(e.to_string())))
            .collect(),
    }
}
