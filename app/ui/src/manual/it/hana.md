# SAP HANA

NiceDB parla il protocollo di SAP HANA da solo: non serve il client di SAP. Questo motore non è ancora stato provato su un server vero.

## Connettersi
1. **Nuova connessione**, poi in **Motore** scegli SAP HANA.
2. **Host** e **porta**: 30015 per un sistema con un solo database, la porta SQL del tenant per un sistema multitenant (per esempio 30041).
3. **Tenant**: il nome del database in un sistema multitenant; vuoto per il database della porta.
4. **Utente** e **password**; la password sta nel portachiavi.
5. **TLS**: con un certificato CA indica il file, NiceDB usa la sua cartella; senza, valgono le autorità di certificazione comuni.

## Esplorare
Sotto la connessione ci sono gli schemi, il tuo per primo; restano fuori quelli di sistema. In ogni schema tabelle (con le righe), viste, procedure e funzioni, sequenze e trigger. **Mostra DDL** usa `GET_OBJECT_DEFINITION` di HANA.

## Query e transazioni
Le istruzioni vanno in commit automatico. `SET TRANSACTION` apre una transazione, `COMMIT` o `ROLLBACK` la chiudono. Il DDL conferma da sé. **Ferma** annulla la query dalla seconda connessione (`ALTER SYSTEM CANCEL SESSION`).

## Spiega e strumenti
**Spiega** usa `EXPLAIN PLAN` e disegna gli operatori con righe e costi stimati. Gli strumenti leggono le viste `M_`: **Attività** (connessioni, istruzioni attive, blocchi), **Monitoraggio** (connessioni, transazioni, dimensione dei dati, istruzioni costose dalla plan cache), **Spazio** (dimensione delle tabelle) e **Utenti** (utenti, ruoli e privilegi).
