# Motori compatibili

TimescaleDB, YugabyteDB, Neon e Supabase parlano il protocollo di PostgreSQL; TiDB e PlanetScale quello di MySQL. NiceDB li tratta come motori a sé: marchio, porta, valori di partenza, e solo gli strumenti che funzionano davvero.

## Collegarsi
1. Nel form della connessione scegli il prodotto tra i **Motori**: sta subito dopo PostgreSQL o MySQL.
2. Incolla l'URL del pannello del servizio in **URL di connessione**: host, porta, database e utente si compilano, la password va nel portachiavi. Oppure compila i campi a mano; i segnaposto mostrano come sono fatti di solito.
3. Premi **Prova**: la risposta dice il prodotto e la sua versione, per esempio «TimescaleDB 2.31.0 · gestionale».

| Prodotto | Porta | Note |
|---|---|---|
| TimescaleDB | 5432 | |
| YugabyteDB | 5433 | YSQL; utente e database `yugabyte` |
| Neon | 5432 | TLS obbligatorio; `-pooler` nell'host passa dal pooler |
| Supabase | 5432, 6543 | TLS obbligatorio; 6543 è il pooler per transazione |
| TiDB | 4000 | |
| PlanetScale | 3306 | TLS obbligatorio; una password per ramo |

Collegandosi come semplice PostgreSQL o MySQL a uno di questi server, NiceDB lo riconosce da solo dalla versione, dalle estensioni o dal nome dell'host.

## La scheda del motore
In **Strumenti**, sotto l'elenco, la scheda del motore dice il prodotto e la versione, su quale motore poggia e quello che il servizio rivela: endpoint, regione, ramo e pooler di Neon, progetto di Supabase, nodi di YugabyteDB e TiDB. **Da sapere** elenca i limiti che cambiano il lavoro, per esempio che dal pooler le impostazioni di sessione non sopravvivono alla transazione.

## TimescaleDB
- Le ipertabelle portano l'etichetta **iper** nell'albero; il suggerimento dice chunk e chunk compressi.
- Gli **Aggregati continui** hanno il loro gruppo; gli schemi di TimescaleDB stanno raccolti sotto **TimescaleDB**, quelli interni non compaiono.
- **Mostra DDL** aggiunge `create_hypertable`, la compressione e i criteri di compressione e conservazione; un aggregato continuo esce con la sua query originale.
- **Strumenti › Spazio occupato** conta tutti i chunk, mostra quanto ha liberato la compressione e l'elenco dei chunk con l'etichetta **compresso**.

## Supabase e la sicurezza per riga
Gli schemi della piattaforma (`auth`, `storage`, `realtime`, `vault`…) stanno raccolti sotto **Supabase**, separati da quelli del progetto: meglio non modificarli. Su ogni PostgreSQL le tabelle con la sicurezza per riga attiva portano l'etichetta **RLS**, e **Mostra DDL** scrive anche `ENABLE ROW LEVEL SECURITY` e le regole (`CREATE POLICY`).

## TiDB e PlanetScale
- TiDB: niente procedure e trigger; **Spiega** legge i piani di TiDB, il monitoraggio usa le tabelle di TiDB per connessioni, transazioni, attese sui lock e query lente.
- PlanetScale: **Utenti e permessi**, **Backup e ripristino** e **Sicurezza** non sono disponibili, perché stanno nel pannello del servizio. Sul ramo di produzione lo schema cambia con le richieste di deploy, e le chiavi esterne valgono solo se il database le ha attivate.

## Limiti
- Neon, Supabase e PlanetScale sono riconosciuti dal nome dell'host e dalle impostazioni del server; dietro un dominio proprio vanno scelti nel form.
- Il backup di TimescaleDB con `pg_dump` va ripristinato con le procedure di TimescaleDB.
- Su YugabyteDB e TiDB i lock sono distribuiti: il monitoraggio vede le attese, non l'albero completo.
