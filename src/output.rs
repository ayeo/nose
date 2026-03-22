use std::io::Write;
use crate::event::Event;

pub fn write_events_jsonl(events: &[Event], writer: &mut impl Write) -> std::io::Result<()> {
    for event in events {
        serde_json::to_writer(&mut *writer, event)?;
        writeln!(writer)?;
    }
    Ok(())
}
