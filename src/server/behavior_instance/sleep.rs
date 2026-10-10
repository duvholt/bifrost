use std::time::Duration;

use async_trait::async_trait;
use chrono::NaiveTime;
use hue::api::GoToSleepConfiguration;

use crate::{
    error::{ApiError, ApiResult},
    server::behavior_instance::{
        BehaviorContext,
        schedule::{Schedule, ScheduledBehavior},
        service::ScheduleType,
    },
};

pub struct GoToSleepBehavior {
    pub configuration: GoToSleepConfiguration,
}

#[async_trait]
impl ScheduledBehavior for GoToSleepBehavior {
    fn name(&self) -> &'static str {
        "go to sleep"
    }

    fn schedule(&self) -> ApiResult<Schedule> {
        let weekdays = self.configuration.when.recurrence_days.as_ref();

        #[allow(clippy::option_if_let_else)]
        let schedule_type = match weekdays {
            Some(weekdays) => ScheduleType::Recurring(weekdays.iter().copied().collect()),
            None => ScheduleType::Once(),
        };

        let job_time = self.configuration.when.time_point.time();
        let scheduled_wakeup_time = NaiveTime::from_hms_opt(job_time.hour, job_time.minute, 0)
            .ok_or(ApiError::InvalidNaiveTime)?;

        Ok(Schedule {
            recurrence: schedule_type,
            scheduled_time: scheduled_wakeup_time,
            lead_time: Duration::ZERO,
        })
    }

    async fn execute(&self, ctx: &BehaviorContext) -> ApiResult<()> {
        log::info!("Starting go to sleep");
        Ok(())
    }
}
