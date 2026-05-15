package com.busarrival.app.service

import kotlinx.serialization.Serializable

/**
 * Single tick of pipeline state for trace output.
 * JSONL format: one JSON object per line.
 *
 * Compatible with Rust trace format for golden test validation.
 */
@Serializable
data class TraceTick(
    val time: Long,           // GPS timestamp (ms)
    val s_cm: Long,           // Route position (cm)
    val off_route: Boolean,   // Off-route mode flag
    val stop_states: List<StopStateEntry>?  // Per-stop FSM states (null if empty)
)

/**
 * Single stop's FSM state entry in trace.
 */
@Serializable
data class StopStateEntry(
    val stop_idx: Int,           // Stop index
    val fsm_state: String,       // FSM state name ("Approaching", "Arriving", "AtStop", "Departed", "Idle")
    val skip_on_reentry: Boolean = false  // Skip flag for recovery
)
