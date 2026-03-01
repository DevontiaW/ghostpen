export interface TextChange {
  start: number;
  end: number;
  original: string;
  replacement: string;
}

export interface AiCorrectionResult {
  original: string;
  corrected: string;
  changes: TextChange[];
}
