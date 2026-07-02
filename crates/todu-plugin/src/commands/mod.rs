mod add;
mod branch;
mod clear;
mod edit;
mod get;
mod list;
mod r#move;
mod priority;
mod rm;
#[cfg(feature = "remote")]
mod pull;
#[cfg(feature = "remote")]
mod remote;
mod status;

pub use add::ToduAdd;
pub use branch::ToduBranch;
pub use clear::ToduClear;
pub use edit::{ToduDesc, ToduDue, ToduTag, ToduTitle};
pub use get::ToduGet;
pub use list::ToduList;
pub use r#move::ToduMove;
pub use priority::ToduPriorityCmd;
pub use rm::ToduRm;
#[cfg(feature = "remote")]
pub use pull::{ToduPullGitHub, ToduPullJira};
#[cfg(feature = "remote")]
pub use remote::{ToduRemoteAddGitHub, ToduRemoteAddJira, ToduRemoteList, ToduRemoteRm};
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
