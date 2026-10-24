use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Datelike, Days, NaiveDate, NaiveTime, TimeZone};

use crate::error::{ApiError, ApiResult};
use crate::server::behavior_instance::BehaviorClock;
use crate::server::behavior_instance::service::ScheduleType;

#[derive(Debug)]
pub struct Schedule {
    pub recurrence: ScheduleType,
    pub scheduled_time: NaiveTime,
    pub lead_time: Duration,
}

impl Schedule {
    fn candidate_start<Tz: TimeZone>(
        &self,
        scheduled_date: NaiveDate,
        timezone: &Tz,
    ) -> Option<DateTime<Tz>> {
        let Some(scheduled_at) = timezone
            .from_local_datetime(&scheduled_date.and_time(self.scheduled_time))
            .latest()
        else {
            // Time does not exist on this day (e.g. DST jump forward).
            return None;
        };
        Some(scheduled_at - self.lead_time)
    }

    #[allow(clippy::needless_continue)]
    fn next_start<Tz: TimeZone>(&self, now: &DateTime<Tz>) -> ApiResult<DateTime<Tz>> {
        let now_date = now.date_naive();
        // In most cases we shouldn't need to iterate more than 7 days,
        // but in edge cases like leap years we might have to check further into the future.
        // In case of any bugs we don't want to loop forever so it should be safe to just check the next 3 weeks
        for days_to_add in 0..21 {
            let Some(next_date) = now_date.checked_add_days(Days::new(days_to_add)) else {
                // unlikely to happen as we're dealing with a naive date, but let's skip if something weird happens
                continue;
            };
            if let ScheduleType::Recurring(weekdays) = &self.recurrence
                && !weekdays.contains(&next_date.weekday())
            {
                continue;
            }
            if let Some(start) = self.candidate_start(next_date, &now.timezone())
                && &start >= now
            {
                return Ok(start);
            }
        }
        Err(match &self.recurrence {
            ScheduleType::Once() => ApiError::NoNextScheduleStart(self.scheduled_time),
            ScheduleType::Recurring(weekdays) => {
                ApiError::NoNextWeekdayOccurence(self.scheduled_time, weekdays.clone())
            }
        })
    }

    pub async fn wait_until_next_start(&self, clock: Arc<dyn BehaviorClock>) -> ApiResult<()> {
        let now = clock.local_now();
        let start_at = self.next_start(&now)?;
        let time_until_start = (start_at - now).to_std()?;
        clock.sleep(time_until_start).await;
        Ok(())
    }
}

pub async fn wait_for_schedule(schedule: Option<Schedule>, clock: Arc<dyn BehaviorClock>) {
    let result = match &schedule {
        Some(schedule) => schedule.wait_until_next_start(clock.clone()).await,
        None => std::future::pending().await,
    };
    if let Err(err) = result {
        log::error!("Wait until next schedule failed {err} for {schedule:?}");
        std::future::pending::<()>().await;
    }
}

#[cfg(test)]
mod tests {
    use chrono::Weekday;
    use tzfile::ArcTz;

    use super::*;

    const WEDNESDAY_JAN_1: u32 = 1;
    const THURSDAY_JAN_2: u32 = 2;
    const FRIDAY_JAN_3: u32 = 3;
    const SUNDAY_JAN_5: u32 = 5;
    const MONDAY_JAN_6: u32 = 6;
    const NEXT_SUNDAY_JAN_12: u32 = 12;

    fn instant(value: &str) -> DateTime<ArcTz> {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&ArcTz::named("Europe/Oslo").unwrap())
    }

    fn datetime(day: u32, hour: u32, minute: u32) -> DateTime<ArcTz> {
        ArcTz::named("Europe/Oslo")
            .unwrap()
            .with_ymd_and_hms(2020, 1, day, hour, minute, 0)
            .single()
            .unwrap()
    }

    fn schedule(days: Option<&[Weekday]>, hour: u32, minute: u32, lead_minutes: u64) -> Schedule {
        Schedule {
            recurrence: days.map_or_else(ScheduleType::Once, |days| {
                ScheduleType::Recurring(days.iter().copied().collect())
            }),
            scheduled_time: NaiveTime::from_hms_opt(hour, minute, 0).unwrap(),
            lead_time: Duration::from_mins(lead_minutes),
        }
    }

    #[test]
    fn same_day_start_subtracts_lead_time() {
        let schedule = schedule(None, 7, 0, 10);
        assert_eq!(
            schedule
                .next_start(&datetime(WEDNESDAY_JAN_1, 6, 0))
                .unwrap(),
            datetime(WEDNESDAY_JAN_1, 6, 50)
        );
    }

    #[test]
    fn missed_start_skips_today_even_before_scheduled_time() {
        let schedule = schedule(None, 7, 0, 10);
        assert_eq!(
            schedule
                .next_start(&datetime(WEDNESDAY_JAN_1, 6, 55))
                .unwrap(),
            datetime(THURSDAY_JAN_2, 6, 50)
        );
    }

    #[test]
    fn exact_start_runs_immediately() {
        let schedule = schedule(None, 7, 0, 10);
        let now = datetime(WEDNESDAY_JAN_1, 6, 50);
        assert_eq!(schedule.next_start(&now).unwrap(), now);
    }

    #[test]
    fn monday_alarm_starts_on_sunday() {
        let schedule = schedule(Some(&[Weekday::Mon]), 0, 10, 30);
        assert_eq!(
            schedule.next_start(&datetime(SUNDAY_JAN_5, 23, 0)).unwrap(),
            datetime(SUNDAY_JAN_5, 23, 40)
        );
    }

    #[test]
    fn missed_sunday_start_skips_next_monday_alarm() {
        let schedule = schedule(Some(&[Weekday::Mon]), 0, 10, 30);
        assert_eq!(
            schedule
                .next_start(&datetime(SUNDAY_JAN_5, 23, 45))
                .unwrap(),
            datetime(NEXT_SUNDAY_JAN_12, 23, 40)
        );
    }

    #[test]
    fn recurrence_skips_unselected_weekdays() {
        let schedule = schedule(Some(&[Weekday::Wed, Weekday::Fri]), 7, 0, 10);
        assert_eq!(
            schedule
                .next_start(&datetime(THURSDAY_JAN_2, 6, 0))
                .unwrap(),
            datetime(FRIDAY_JAN_3, 6, 50)
        );
    }

    #[test]
    fn empty_recurrence_returns_error() {
        let schedule = schedule(Some(&[]), 7, 0, 10);
        assert!(matches!(
            schedule.next_start(&datetime(WEDNESDAY_JAN_1, 6, 0)),
            Err(ApiError::NoNextWeekdayOccurence(_, days)) if days.is_empty()
        ));
    }

    #[test]
    fn one_time_midnight_start_uses_tomorrows_alarm() {
        let schedule = schedule(None, 0, 10, 30);
        assert_eq!(
            schedule.next_start(&datetime(SUNDAY_JAN_5, 23, 0)).unwrap(),
            datetime(SUNDAY_JAN_5, 23, 40)
        );
    }

    #[test]
    fn recurring_fade_crosses_spring_forward() {
        let schedule = schedule(Some(&[Weekday::Sun]), 3, 10, 30);
        // Sunday, March 29: Oslo jumps from 02:00 CET to 03:00 CEST.
        let now = instant("2020-03-29T00:00:00+01:00");
        // Thirty elapsed minutes before 03:10 CEST is 01:40 CET.
        assert_eq!(
            schedule.next_start(&now).unwrap(),
            instant("2020-03-29T01:40:00+01:00")
        );
    }

    #[test]
    fn recurring_fade_crosses_fall_back() {
        let schedule = schedule(Some(&[Weekday::Sun]), 3, 10, 90);
        // Sunday, October 25: Oslo repeats 02:00 after clocks fall back.
        let now = instant("2020-10-25T00:00:00+02:00");
        // Ninety elapsed minutes before 03:10 CET is the first 02:40.
        assert_eq!(
            schedule.next_start(&now).unwrap(),
            instant("2020-10-25T02:40:00+02:00")
        );
    }

    #[test]
    fn one_time_ambiguous_start_today_uses_later_occurrence() {
        let schedule = schedule(None, 2, 30, 0);
        // Sunday, October 25: Oslo repeats 02:00 after clocks fall back.
        let now = instant("2020-10-25T00:00:00+02:00");
        assert_eq!(
            schedule.next_start(&now).unwrap(),
            instant("2020-10-25T02:30:00+01:00")
        );
    }

    #[test]
    fn one_time_ambiguous_start_tomorrow_uses_later_occurrence() {
        let schedule = schedule(None, 2, 30, 0);
        // Saturday, October 24: tomorrow is the Sunday with two 02:30s.
        let now = instant("2020-10-24T12:00:00+02:00");
        assert_eq!(
            schedule.next_start(&now).unwrap(),
            instant("2020-10-25T02:30:00+01:00")
        );
    }

    #[test]
    fn one_time_tomorrows_fade_crosses_spring_forward() {
        let schedule = schedule(None, 3, 10, 30);
        // Saturday, March 28: tomorrow's 03:10 alarm follows the DST jump.
        let now = instant("2020-03-28T12:00:00+01:00");
        assert_eq!(
            schedule.next_start(&now).unwrap(),
            instant("2020-03-29T01:40:00+01:00")
        );
    }

    #[test]
    fn one_time_missed_sunday_fade_skips_monday_alarm() {
        let schedule = schedule(None, 0, 10, 30);
        // Sunday at 23:45: Monday's 00:10 alarm needed a 23:40 start.
        let now = datetime(SUNDAY_JAN_5, 23, 45);
        assert_eq!(
            schedule.next_start(&now).unwrap(),
            datetime(MONDAY_JAN_6, 23, 40),
        );
    }

    #[test]
    fn one_time_alarm_in_spring_gap_uses_mondays_alarm() {
        let schedule = schedule(None, 2, 30, 0);
        // Sunday, March 29 before the DST jump: today's 02:30 will not exist in Oslo.
        let now = instant("2020-03-29T00:00:00+01:00");
        assert_eq!(
            schedule.next_start(&now).unwrap(),
            // Monday, March 30: 02:30 exists and is the next alarm.
            instant("2020-03-30T02:30:00+02:00")
        );
    }
}
