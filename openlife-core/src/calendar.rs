/// Simple ICS (iCalendar) file parser.
/// Reads VEVENT blocks from .ics files and returns parsed calendar events.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CalendarEvent {
    pub uid: String,
    pub summary: String,
    pub dtstart: String,
    pub dtend: String,
    pub description: String,
    pub location: String,
}

/// Parse ICS content and return events within the specified date range.
/// Returns all events if range_start/range_end are empty.
pub fn parse_ics(
    content: &str,
    range_start: Option<&str>,
    range_end: Option<&str>,
) -> Vec<CalendarEvent> {
    let mut events = Vec::new();
    let mut in_vevent = false;
    let mut current: Option<CalendarEventBuilder> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line == "BEGIN:VEVENT" {
            in_vevent = true;
            current = Some(CalendarEventBuilder::default());
            continue;
        }

        if line == "END:VEVENT" {
            in_vevent = false;
            if let Some(builder) = current.take() {
                if let Some(event) = builder.build() {
                    // Filter by date range if specified
                    if let Some(start) = range_start {
                        if let Some(end) = range_end {
                            if event.dtstart.as_str() >= start && event.dtstart.as_str() <= end {
                                events.push(event);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ics_basic_event() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:test-1\nSUMMARY:Meeting\nDTSTART:20260101T100000Z\nDTEND:20260101T110000Z\nEND:VEVENT\nEND:VCALENDAR";
        let events = parse_ics(ics, None, None);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].summary, "Meeting");
        assert_eq!(events[0].dtstart, "20260101T100000Z");
        assert_eq!(events[0].uid, "test-1");
    }

    #[test]
    fn test_parse_ics_empty() {
        let events = parse_ics("", None, None);
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_ics_date_range() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:early\nSUMMARY:Early\nDTSTART:20250101T000000Z\nEND:VEVENT\nBEGIN:VEVENT\nUID:late\nSUMMARY:Late\nDTSTART:20270101T000000Z\nEND:VEVENT\nEND:VCALENDAR";
        let events = parse_ics(ics, Some("20260101T000000Z"), Some("20261231T235959Z"));
        // Neither early nor late are in 2026 range — both filtered out
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_unescape_ics_text() {
        assert_eq!(unescape_ics_text("hello\\nworld"), "hello\nworld");
        assert_eq!(unescape_ics_text("a\\;b\\,c\\\\d"), "a;b,c\\d");
    }
}
                        } else if event.dtstart.as_str() >= start {
                            events.push(event);
                        }
                    } else if let Some(end) = range_end {
                        if event.dtstart.as_str() <= end {
                            events.push(event);
                        }
                    } else {
                        events.push(event);
                    }
                }
            }
            continue;
        }

        if in_vevent {
            if let Some(ref mut builder) = current {
                apply_property(line, builder);
            }
        }
    }

    events
}

fn apply_property(line: &str, builder: &mut CalendarEventBuilder) {
    if let Some(rest) = line.strip_prefix("UID:") {
        builder.uid = rest.trim().to_string();
    } else if let Some(rest) = line.strip_prefix("SUMMARY:") {
        builder.summary = unescape_ics_text(rest.trim());
    } else if let Some(rest) = line.strip_prefix("DTSTART;") {
        if let Some(val) = rest.split(':').nth(1) {
            builder.dtstart = val.trim().to_string();
        }
    } else if let Some(rest) = line.strip_prefix("DTSTART:") {
        builder.dtstart = rest.trim().to_string();
    } else if let Some(rest) = line.strip_prefix("DTEND;") {
        if let Some(val) = rest.split(':').nth(1) {
            builder.dtend = val.trim().to_string();
        }
    } else if let Some(rest) = line.strip_prefix("DTEND:") {
        builder.dtend = rest.trim().to_string();
    } else if let Some(rest) = line.strip_prefix("DESCRIPTION:") {
        builder.description = unescape_ics_text(rest.trim());
    } else if let Some(rest) = line.strip_prefix("LOCATION:") {
        builder.location = unescape_ics_text(rest.trim());
    }
}

fn unescape_ics_text(text: &str) -> String {
    text.replace("\\n", "\n")
        .replace("\\;", ";")
        .replace("\\,", ",")
        .replace("\\\\", "\\")
}

#[derive(Default)]
struct CalendarEventBuilder {
    uid: String,
    summary: String,
    dtstart: String,
    dtend: String,
    description: String,
    location: String,
}

impl CalendarEventBuilder {
    fn build(self) -> Option<CalendarEvent> {
        if self.summary.is_empty() && self.dtstart.is_empty() {
            return None;
        }
        Some(CalendarEvent {
            uid: if self.uid.is_empty() {
                format!(
                    "generated-{}",
                    chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
                )
            } else {
                self.uid
            },
            summary: self.summary,
            dtstart: self.dtstart,
            dtend: self.dtend,
            description: self.description,
            location: self.location,
        })
    }
}
