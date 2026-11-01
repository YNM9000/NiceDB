# Db2

Per Db2 (Linux, Unix e Windows) serve il driver CLI di IBM, il «clidriver»: è gratuito ma la licenza di IBM non permette di includerlo in NiceDB, quindi si scarica una volta dal sito di IBM.

## Il driver di IBM
1. Scarica il clidriver per il tuo sistema dal sito pubblico di IBM (`odbc_cli`): `linuxx64_odbc_cli.tar.gz`, `ntx64_odbc_cli.zip`, `macarm64_odbc_cli.tar.gz` o `macos64_odbc_cli.tar.gz`. Pesa una ventina di MB e non chiede un account.
2. Scompattalo in una cartella stabile, per esempio `/opt/clidriver` o `C:\clidriver`.
3. Su Windows apri un prompt da amministratore e lancia `clidriver\bin\db2cli install -setup`, che registra il driver.
4. In **Impostazioni › Driver** scegli la cartella `clidriver`. Se il driver manca o la cartella è sbagliata, la connessione lo dice; gli altri motori funzionano comunque.

## Connettersi
**Nuova connessione**, **Motore** Db2: host, porta (50000), **Database** (per esempio `GESTION`), utente e password. Con **TLS** NiceDB chiede SSL al server; il **Certificato CA** è il file del certificato del server. Tunnel SSH e proxy come per gli altri motori.

## Esplorare e interrogare
Sotto la connessione ci sono gli schemi, il tuo per primo; quelli di Db2 (`SYS…`, `NULLID`, `SQLJ`) restano fuori. In ogni schema tabelle con le righe delle statistiche, viste, tabelle materializzate, routine, sequenze e trigger, letti da `SYSCAT`. **Mostra DDL** per tabelle, viste, routine, trigger, indici e sequenze; **Dipendenze** da `SYSCAT.TABDEP` e dalle chiavi esterne.

Le istruzioni vanno in commit automatico; `BEGIN TRANSACTION` lo spegne fino a `COMMIT` o `ROLLBACK`. I blocchi SQL PL (`BEGIN … END`, procedure, trigger) restano interi. **Ferma** annulla la query in corso.

## Spiega e strumenti
**Spiega** usa `EXPLAIN PLAN` e le tabelle di explain del tuo schema: se mancano, Db2 lo dice e si creano con `CALL SYSPROC.SYSINSTALLOBJECTS('EXPLAIN', 'C', NULL, CURRENT SCHEMA)`. Gli strumenti usano le funzioni `MON_GET_*`: **Attività** con i blocchi e la chiusura di una connessione (`FORCE APPLICATION`), **Monitoraggio** (connessioni, transazioni, dimensione, buffer pool, istruzioni costose), **Spazio** (`ADMINTABINFO`, uso degli indici) e **Utenti** (autorizzazioni, ruoli, permessi sulle tabelle).
