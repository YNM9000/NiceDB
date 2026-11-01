import type { KeyedRelation } from './nosql2-lang';

export interface DynamoAsk {
  connectionId: string;
  database: string;
  rel: KeyedRelation;
  index: string | null;
}

export const dynamoUi = $state({ ask: null as DynamoAsk | null });

export const openDynamoBuilder = (ask: DynamoAsk) => (dynamoUi.ask = ask);
