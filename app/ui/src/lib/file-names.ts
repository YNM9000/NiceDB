import { m } from '../paraglide/messages';
import { plainName } from './tools';

export const resultFile = (label: string, ext: string) => `${plainName(label) || m.file_result()}.${ext}`;
export const erFile = (database: string, ext: 'svg' | 'png') => `${m.file_er({ database: database || m.file_schema() })}.${ext}`;
export const backupBase = (database: string, date: string) => `${database || m.file_backup()}-${date}`;
export const migrationFile = (schema: string) => `${m.file_migration({ schema: schema || m.file_schema() })}.sql`;
export const rowDiffFile = (table: string) => `${m.file_row_diff({ table: table || m.file_data() })}.sql`;
