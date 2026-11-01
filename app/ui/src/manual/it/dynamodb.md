# DynamoDB

DynamoDB tiene gli elementi in tabelle con una chiave di partizione e, a volte, una di ordinamento. NiceDB li legge con PartiQL o con le chiamate dell'API scritte in JSON, e li mostra come documenti.

## Connettersi
1. **Nuova connessione**, poi in **Motore** scegli DynamoDB.
2. **Regione**, per esempio `eu-west-1`.
3. **Endpoint**: vuoto usa quello della regione; per DynamoDB Local scrivi `http://127.0.0.1:8000`.
4. Le credenziali vengono dal **Profilo AWS** (letto da `~/.aws/credentials`), oppure da **Access key ID** e **Secret access key**, con il segreto nel portachiavi. Senza nessuno dei due NiceDB usa le variabili d'ambiente di AWS.

## Esplorare
Sotto la connessione ci sono le **Tabelle** della regione, con il numero di elementi. Aprendone una si vedono la chiave (partizione e ordinamento), gli attributi trovati leggendo i primi 50 elementi con il loro tipo (`S`, `N`, `SS`…; se un attributo cambia tipo, la quota di ciascuno), e gli indici globali e locali.

Tasto destro su una tabella:
- **Scan o Query…** apre il costruttore: scegli Query o Scan, la tabella o un indice, il valore della chiave di partizione, una condizione sulla chiave di ordinamento, i filtri, il limite e l'ordine. Sotto vedi la chiamata che ne esce; **Esegui** la apre in una scheda nuova, **Apri nell'editor** la lascia da ritoccare;
- **Scan della tabella** scrive una chiamata `Scan`;
- **Query per chiave** scrive una `Query` sulla chiave della tabella, con i segnaposto da riempire;
- **Query sull'indice …** fa lo stesso per ogni indice;
- **Mostra DDL** dà la chiamata `CreateTable` che la ricrea vuota.

## Scrivere
L'editor accetta due cose, una dopo l'altra:
- istruzioni PartiQL che finiscono con `;`: `SELECT * FROM "Catalogo" WHERE articolo = 'ART-104';`
- chiamate dell'API come un oggetto JSON con il nome dell'operazione:

```json
{ "Query": { "TableName": "Catalogo",
  "KeyConditionExpression": "#pk = :pk",
  "ExpressionAttributeNames": { "#pk": "articolo" },
  "ExpressionAttributeValues": { ":pk": { "S": "ART-104" } } } }
```

`Scan` e `Query` seguono le pagine fino al limite di righe; con `Limit` ne leggono una sola. **Formatta** riscrive su più righe le chiamate JSON.

In PartiQL i nomi di tabelle e attributi vanno tra virgolette doppie e le stringhe tra apici; NiceDB mette sempre le virgolette ai nomi che scrive, perché DynamoDB riserva centinaia di parole (`status`, `name`, `size`…). PartiQL non ha `LIMIT`: le pagine le decide DynamoDB. Dei comandi rapidi del pannello **Note** resta `ins`, che scrive un `INSERT INTO "tabella" VALUE {…}`.

## Risultati
Ogni elemento è un documento, con gli attributi della chiave per primi. I numeri restano numeri JSON quando si possono rileggere esatti, altrimenti diventano `{ "$numberDecimal": "…" }`; gli insiemi sono `{ "$set": [...] }`; i binari `{ "$binary": … }`. Così una modifica rimette ogni attributo con il suo tipo. Nella **Tabella**, **Copia come › INSERT** scrive una `INSERT INTO … VALUE {…}` per elemento, con gli insiemi come `<<…>>`.

## Modificare gli elementi
Gli elementi di un `SELECT *` o di uno `Scan`/`Query` senza proiezione si modificano per chiave: **Salva** riscrive l'elemento solo se c'è ancora, **Inserisci** solo se la chiave è libera, **Elimina** solo se l'elemento esiste. In produzione ogni scrittura passa dal controllo delle scritture e finisce nel registro come chiamata `PutItem` o `DeleteItem`.

## Scritture e produzione
`SELECT` e le chiamate `Scan`, `Query`, `GetItem`, `BatchGetItem`, `Describe…` e `List…` sono letture; il resto scrive. `DeleteTable` e `UpdateTable` chiedono conferma come rischiose.

## Strumenti › Cluster
Regione, endpoint, tabelle, elementi e spazio in tutto; per ogni tabella stato, elementi, spazio, capacità (on demand o RCU e WCU), chiave e numero di indici. Il numero di elementi è quello che DynamoDB aggiorna circa ogni sei ore.
