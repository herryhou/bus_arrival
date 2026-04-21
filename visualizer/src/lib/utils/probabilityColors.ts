export interface ProbabilityColor {
  color: string;
  label: string;
}

export function getProbabilityColor(probability: number): string {
  if (probability < 64) return '#666666';      // Gray - no chance
  if (probability < 128) return '#eab308';     // Yellow - low
  if (probability < 191) return '#f97316';     // Orange - medium
  return '#22c55e';                             // Green - high/arrived
}

export function getProbabilityLabel(probability: number): string {
  if (probability < 64) return 'None';
  if (probability < 128) return 'Low';
  if (probability < 191) return 'Medium';
  return 'High';
}

export function getProbabilityTextColor(probability: number): string {
  if (probability < 191) return '#ffffff';
  return '#000000';
}
