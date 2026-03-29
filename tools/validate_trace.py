#!/usr/bin/env python3
"""
Trace validation tool for bus arrival detection system.

Analyzes {route}_trace.jsonl files and reports:
- Overall system health
- Missing events per stop
- Position accuracy (distance when AtStop)
- Dwell time estimation
"""

import json
import sys
from pathlib import Path
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Set
from collections import defaultdict


@dataclass
class StopEvent:
    """Represents a state change event for a stop."""
    time: int
    state: str
    s_cm: int
    v_cms: int
    distance_cm: int
    probability: int


@dataclass
class StopAnalysis:
    """Analysis results for a single stop."""
    stop_idx: int
    events: Dict[str, StopEvent] = field(default_factory=dict)
    has_approaching: bool = False
    has_arriving: bool = False
    has_at_stop: bool = False
    has_departed: bool = False

    # Timing info
    first_seen_time: Optional[int] = None
    at_stop_first_time: Optional[int] = None
    at_stop_last_time: Optional[int] = None
    departed_time: Optional[int] = None

    # Position info
    at_stop_distance_cm: Optional[int] = None
    at_stop_speed_cms: Optional[int] = None

    # AtStop sequence
    at_stop_records: List[Dict] = field(default_factory=list)

    # Ground truth comparison (for dwell time only - timestamps are incompatible)
    gt_dwell_s: Optional[int] = None

    # Issues
    issues: List[str] = field(default_factory=list)

    @property
    def is_complete(self) -> bool:
        """Check if stop has complete FSM cycle."""
        return self.has_approaching and self.has_arriving and self.has_at_stop and self.has_departed

    @property
    def dwell_time_s(self) -> Optional[int]:
        """Calculate dwell time from AtStop duration."""
        if self.at_stop_first_time and self.at_stop_last_time:
            return self.at_stop_last_time - self.at_stop_first_time
        return None


@dataclass
class ValidationResult:
    """Overall validation results."""
    trace_file: str
    total_records: int
    time_range: tuple[int, int]

    # Stop analysis
    stops_analyzed: Dict[int, StopAnalysis] = field(default_factory=dict)
    missing_stops: Set[int] = field(default_factory=set)

    # Ground truth
    gt_stops: Set[int] = field(default_factory=set)

    # Issues
    global_issues: List[str] = field(default_factory=list)

    @property
    def total_stops(self) -> int:
        return len(self.stops_analyzed)

    @property
    def complete_stops(self) -> int:
        return sum(1 for s in self.stops_analyzed.values() if s.is_complete)

    @property
    def stops_with_at_stop(self) -> int:
        return sum(1 for s in self.stops_analyzed.values() if s.has_at_stop)


def parse_trace(trace_path: Path) -> ValidationResult:
    """Parse trace.jsonl file."""
    records = []
    with open(trace_path) as f:
        for line in f:
            line = line.strip()
            if line:
                records.append(json.loads(line))

    if not records:
        raise ValueError(f"No records found in {trace_path}")

    time_range = (records[0]['time'], records[-1]['time'])

    result = ValidationResult(
        trace_file=str(trace_path),
        total_records=len(records),
        time_range=time_range
    )

    # Analyze each record
    for record in records:
        time = record['time']
        s_cm = record['s_cm']
        v_cms = record['v_cms']

        for stop_state in record.get('stop_states', []):
            stop_idx = stop_state['stop_idx']
            state = stop_state['fsm_state']
            distance_cm = stop_state['distance_cm']
            probability = stop_state['probability']

            if stop_idx not in result.stops_analyzed:
                result.stops_analyzed[stop_idx] = StopAnalysis(stop_idx=stop_idx)

            analysis = result.stops_analyzed[stop_idx]

            # Track first seen
            if analysis.first_seen_time is None:
                analysis.first_seen_time = time

            # Track state changes
            if state == 'Approaching' and not analysis.has_approaching:
                analysis.has_approaching = True
                analysis.events['Approaching'] = StopEvent(
                    time, state, s_cm, v_cms, distance_cm, probability
                )
            elif state == 'Arriving' and not analysis.has_arriving:
                analysis.has_arriving = True
                analysis.events['Arriving'] = StopEvent(
                    time, state, s_cm, v_cms, distance_cm, probability
                )
            elif state == 'AtStop':
                if not analysis.has_at_stop:
                    analysis.has_at_stop = True
                    analysis.at_stop_first_time = time
                    analysis.at_stop_distance_cm = distance_cm
                    analysis.at_stop_speed_cms = v_cms

                # Track last AtStop time
                analysis.at_stop_last_time = time

                # Store AtStop record
                analysis.at_stop_records.append({
                    'time': time,
                    'distance_cm': distance_cm,
                    'v_cms': v_cms,
                    's_cm': s_cm
                })

                # Always update AtStop event (most recent)
                analysis.events['AtStop'] = StopEvent(
                    time, state, s_cm, v_cms, distance_cm, probability
                )
            elif state == 'Departed' and not analysis.has_departed:
                analysis.has_departed = True
                analysis.departed_time = time
                analysis.events['Departed'] = StopEvent(
                    time, state, s_cm, v_cms, distance_cm, probability
                )

    return result


def load_ground_truth(gt_path: Path, result: ValidationResult):
    """Load ground truth for dwell time comparison."""
    with open(gt_path) as f:
        gt_data = json.load(f)

    for gt_stop in gt_data:
        stop_idx = gt_stop['stop_idx']
        result.gt_stops.add(stop_idx)

        if stop_idx in result.stops_analyzed:
            analysis = result.stops_analyzed[stop_idx]
            analysis.gt_dwell_s = gt_stop['dwell_s']

    # Find stops in ground truth but not in trace
    result.missing_stops = result.gt_stops - set(result.stops_analyzed.keys())


def analyze_issues(result: ValidationResult):
    """Analyze and categorize issues."""
    departed_count = sum(1 for s in result.stops_analyzed.values() if s.has_departed)

    # Global issue: no Departed states at all
    if departed_count == 0 and result.total_stops > 0:
        result.global_issues.append("CRITICAL: FSM never transitions to Departed state (state machine bug)")

    for stop_idx, analysis in result.stops_analyzed.items():
        # Check for missing states (except Departed since it's a global issue)
        if not analysis.has_approaching:
            analysis.issues.append("Missing Approaching state")
        if not analysis.has_arriving:
            analysis.issues.append("Missing Arriving state")
        if not analysis.has_at_stop:
            analysis.issues.append("Missing AtStop state - CRITICAL")

        # Check position accuracy at AtStop
        if analysis.at_stop_distance_cm is not None:
            if abs(analysis.at_stop_distance_cm) > 5000:  # More than 50m off
                analysis.issues.append(f"Poor position accuracy: {analysis.at_stop_distance_cm/100:.1f}m from stop")

        # Check dwell time vs ground truth
        if analysis.dwell_time_s and analysis.gt_dwell_s:
            dwell_error = analysis.dwell_time_s - analysis.gt_dwell_s
            if abs(dwell_error) > 3:  # More than 3 seconds off
                analysis.issues.append(f"Dwell time error: {dwell_error:+d}s (expected {analysis.gt_dwell_s}s, got {analysis.dwell_time_s}s)")

    # Global issues
    if result.missing_stops:
        result.global_issues.append(f"Missing {len(result.missing_stops)} stops from ground truth: {sorted(result.missing_stops)}")


def print_report(result: ValidationResult, verbose: bool = False):
    """Print validation report."""
    print("=" * 70)
    print(f"TRACE VALIDATION REPORT: {result.trace_file}")
    print("=" * 70)

    # Summary
    print(f"\n📊 SUMMARY")
    print(f"  Total records:     {result.total_records}")
    print(f"  Time range:        {result.time_range[0]}s - {result.time_range[1]}s ({result.time_range[1]-result.time_range[0]}s total)")
    print(f"  Stops analyzed:    {result.total_stops}")
    print(f"  With AtStop:       {result.stops_with_at_stop}/{result.total_stops}")
    print(f"  With Departed:     {sum(1 for s in result.stops_analyzed.values() if s.has_departed)}/{result.total_stops}")

    if result.gt_stops:
        print(f"  Ground truth stops: {len(result.gt_stops)}")
        if result.missing_stops:
            print(f"  ⚠️  Missing stops:    {len(result.missing_stops)} - {sorted(result.missing_stops)}")

    # Global issues
    if result.global_issues:
        print(f"\n⚠️  GLOBAL ISSUES")
        for issue in result.global_issues:
            print(f"  - {issue}")

    # Calculate health based on AtStop detection (Departed is a known bug)
    at_stop_ratio = result.stops_with_at_stop / result.total_stops if result.total_stops else 0
    if at_stop_ratio >= 0.95:
        print(f"\n✅ SYSTEM HEALTH: EXCELLENT ({at_stop_ratio*100:.1f}% stops detected)")
    elif at_stop_ratio >= 0.80:
        print(f"\n🟡 SYSTEM HEALTH: GOOD ({at_stop_ratio*100:.1f}% stops detected)")
    elif at_stop_ratio >= 0.50:
        print(f"\n🟠 SYSTEM HEALTH: FAIR ({at_stop_ratio*100:.1f}% stops detected)")
    else:
        print(f"\n🔴 SYSTEM HEALTH: POOR ({at_stop_ratio*100:.1f}% stops detected)")

    # Per-stop details
    print(f"\n📍 STOP DETAILS")

    # Sort by stop index
    for stop_idx in sorted(result.stops_analyzed.keys()):
        analysis = result.stops_analyzed[stop_idx]

        # Status indicator (based on AtStop since Departed is broken)
        if analysis.is_complete:
            status = "✅"
        elif analysis.has_at_stop:
            status = "🟢"
        elif analysis.has_arriving:
            status = "🟠"
        elif analysis.has_approaching:
            status = "⚪"
        else:
            status = "❌"

        print(f"\n  {status} Stop {stop_idx:2d}:", end="")

        # FSM states
        states = []
        if analysis.has_approaching:
            e = analysis.events.get('Approaching')
            states.append(f"Approaching(t={e.time if e else '?'})")
        if analysis.has_arriving:
            e = analysis.events.get('Arriving')
            states.append(f"Arriving(t={e.time if e else '?'})")
        if analysis.has_at_stop:
            dur = f"~{analysis.dwell_time_s}s" if analysis.dwell_time_s else "?"
            states.append(f"AtStop(t={analysis.at_stop_first_time}..{analysis.at_stop_last_time}, {dur})")
            if analysis.at_stop_distance_cm is not None:
                states.append(f"dist={analysis.at_stop_distance_cm/100:.1f}m")
        if analysis.has_departed:
            states.append(f"Departed(t={analysis.departed_time})")

        if states:
            print(" " + ", ".join(states))
        else:
            print(" No states recorded")

        # Issues
        if analysis.issues:
            for issue in analysis.issues:
                print(f"      ⚠️  {issue}")

        # Verbose details
        if verbose:
            if analysis.gt_dwell_s:
                print(f"      Ground truth dwell: {analysis.gt_dwell_s}s")
            if analysis.at_stop_records:
                # Show position progression during AtStop
                if len(analysis.at_stop_records) > 1:
                    first_dist = analysis.at_stop_records[0]['distance_cm']
                    last_dist = analysis.at_stop_records[-1]['distance_cm']
                    print(f"      Position during stop: {first_dist/100:.1f}m → {last_dist/100:.1f}m (moved {abs(last_dist-first_dist)/100:.1f}m)")


def main():
    if len(sys.argv) < 2:
        print("Usage: validate_trace.py <trace.jsonl> [ground_truth.json] [--verbose]")
        sys.exit(1)

    trace_path = Path(sys.argv[1])
    if not trace_path.exists():
        print(f"Error: Trace file not found: {trace_path}")
        sys.exit(1)

    gt_path = None
    verbose = False

    for arg in sys.argv[2:]:
        if arg == "--verbose":
            verbose = True
        elif Path(arg).exists():
            gt_path = Path(arg)

    print(f"🔍 Analyzing trace: {trace_path}")
    if gt_path:
        print(f"📋 Ground truth: {gt_path}")
    print()

    result = parse_trace(trace_path)

    if gt_path:
        load_ground_truth(gt_path, result)

    analyze_issues(result)
    print_report(result, verbose=verbose)


if __name__ == "__main__":
    main()
