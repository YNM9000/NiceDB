import { version } from '../../package.json';

export const shortVersion = version.split('.').slice(0, 2).join('.');
