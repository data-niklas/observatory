use std::path::Path;

use rusqlite::{params, Connection, Result};

use crate::model::{
    MonitoringTarget, MonitoringTargetDescriptor, MonitoringTargetStatus,
    MonitoringTargetTypeDescriptor, Observation, ObservedMonitoringTargetStatus,
};
use rocket::serde::json::serde_json;

pub fn init_db(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS monitoring_targets (
            id TEXT PRIMARY KEY,
            interval INTEGER NOT NULL,
            retries INTEGER NOT NULL,
            timeout INTEGER NOT NULL,
            target TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS observations (
            monitoring_target_id TEXT,
            timestamp TEXT,
            status TEXT NOT NULL,
            retries INTEGER NOT NULL,
            PRIMARY KEY (monitoring_target_id, timestamp),
            FOREIGN KEY (monitoring_target_id) REFERENCES monitoring_targets (id)
        )",
        [],
    )?;
    Ok(conn)
}

pub fn add_observation(conn: &Connection, observation: &Observation) -> Result<()> {
    let mut insert_observation = conn.prepare(
        "INSERT INTO observations (monitoring_target_id, timestamp, status, retries)
        VALUES (?, ?, ?, ?)",
    )?;
    let observed_status = &observation.observed_status;
    insert_observation.execute((
        &observation.monitoring_target.name,
        observed_status.timestamp.to_rfc3339(),
        serde_json::to_string(&observed_status.status).unwrap(),
        observed_status.retries,
    ))?;
    Ok(())
}

pub fn get_monitoring_target_descriptors(conn: &Connection) -> Result<Vec<MonitoringTargetDescriptor>> {
    let mut stmt =
        conn.prepare("SELECT id, interval, retries, timeout, target FROM monitoring_targets")?;
    let monitoring_targets_iter = stmt.query_map([], |row| {
        let target_text: String = row.get(4)?;
        Ok(MonitoringTargetDescriptor {
            name: row.get(0)?,
            interval: row.get(1)?,
            retries: row.get(2)?,
            timeout: row.get(3)?,
            target: serde_json::from_str(&target_text).unwrap(),
        })
    })?;
    monitoring_targets_iter.collect::<Result<Vec<MonitoringTargetDescriptor>>>()
}

pub fn has_monitoring_target(conn: &Connection, name: &str) -> Result<bool> {
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM monitoring_targets WHERE id = ?")?;
    let count: i64 = stmt.query_row(params![name], |row| row.get(0))?;
    Ok(count > 0)
}

pub fn get_last_observations(
    conn: &Connection,
    names: &Vec<String>,
) -> Result<Vec<ObservedMonitoringTargetStatus>> {
    let mut result = vec![];
    for name in names.iter() {
        let mut stmt = conn.prepare(
            "SELECT timestamp, status, retries FROM observations
            WHERE monitoring_target_id = ?
            ORDER BY timestamp DESC
            LIMIT 1",
        )?;
        let observation_iter = stmt.query_map(params![name], |row| {
            Ok(ObservedMonitoringTargetStatus {
                timestamp: row.get(0)?,
                status: serde_json::from_str(&row.get::<_, String>(1)?).unwrap(),
                retries: row.get(2)?,
            })
        })?;
        for observation in observation_iter {
            result.push(observation.unwrap());
        }
    }
    Ok(result)
}

pub fn create_or_update_monitoring_target(
    conn: &Connection,
    monitoring_target: &MonitoringTargetDescriptor,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT OR REPLACE INTO monitoring_targets (id, interval, retries, timeout, target)
        VALUES (?, ?, ?, ?, ?)",
    )?;
    let target_text = serde_json::to_string(&monitoring_target.target).unwrap();
    stmt.execute((
        &monitoring_target.name,
        monitoring_target.interval,
        monitoring_target.retries,
        monitoring_target.timeout,
        target_text,
    ))?;
    Ok(())
}
