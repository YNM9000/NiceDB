# ClickHouse

ClickHouse si usa come gli altri motori SQL, con il suo dialetto. Le righe si leggono e si analizzano; non si modificano una per una.

## Connettersi
1. **Nuova connessione**, poi in **Motore** scegli ClickHouse.
2. **Host** e **Porta**: NiceDB usa l'interfaccia HTTP, 8123 in chiaro e 8443 con TLS. Finché non la tocchi, la porta segue la scelta del TLS. La porta nativa 9000 non serve.
3. **Utente**: vuoto vale `default`. La password sta nel portachiavi.
4. **Database**: quello su cui lavorano le query; vuoto vale `default`.
5. **TLS**: **Verifica il certificato** usa HTTPS con i certificati del sistema o con il **Certificato CA (facoltativo)**; **Cifrato, senza verifica** accetta qualsiasi certificato.
6. **Prova**, poi **Salva**.

Tunnel SSH e proxy funzionano come per gli altri server; con il tunnel il certificato si verifica sul nome dell'host scritto nel form. **Copia come URL** dal tasto destro sulla connessione dà `http://utente@host:8123/?database=…`.

## Sessione
Ogni scheda ha una sessione sul server che dura 30 minuti di inattività. Le impostazioni scritte con `SET` e le tabelle temporanee restano tra un'istruzione e l'altra. Le query di init della connessione girano lì, per esempio:

```sql
SET max_threads = 4;
SET session_timezone = 'Europe/Rome';
```

## Esplorare
Sotto la connessione ci sono i database; in ognuno:
- **Tabelle**, con il numero di righe. Passando sopra una tabella si vede il motore (`MergeTree`, `ReplacingMergeTree`…). Aprendola: colonne, chiave primaria evidenziata, **ORDER BY**, **PRIMARY KEY**, **PARTITION BY**, **SAMPLE BY** e gli indici di salto.
- **Viste** e **viste materializzate**, con la tabella in cui scrivono nel suggerimento.
- **Dizionari**, con elementi caricati, layout e stato.
- **Funzioni** definite dall'utente, se ce ne sono.

Sotto il server ci sono le sessioni attive.

Tasto destro su una tabella: **Mostra DDL**, **Copia CREATE**, **Genera SQL**, **Dipendenze…**, esporta e importa, **Ricostruisci la tabella** (`OPTIMIZE TABLE … FINAL`, che passa dal controllo della produzione).

## Scrivere query
- L'autocompletamento propone tabelle, colonne, tipi (`UInt32`, `LowCardinality`…) e le funzioni di ClickHouse con le maiuscole giuste (`toStartOfMonth`, `uniqExact`), perché in ClickHouse contano.
- [[key:format]] usa le regole di ClickHouse.
- I parametri `:nome` e `${anno}` funzionano come altrove: vedi [Parametri nelle query](parameters).
- Un'istruzione con un `FORMAT` proprio (`SELECT … FORMAT CSV`) arriva come testo, una riga del server per riga della griglia.
- Un errore di sintassi è sottolineato nel punto indicato dal server; il messaggio porta il nome dell'errore (`SYNTAX_ERROR`, `UNKNOWN_TABLE`…).

## Seguire una query lunga
Le righe arrivano mentre il server le produce: le prime subito, poi a blocchi. I numeri a 64 bit e i decimali arrivano esatti; array e mappe come JSON.

Quando una query dura più di un istante, sopra i risultati compare l'avanzamento, per esempio «Lette 1,4 Mln di 3,2 Mln righe», con byte letti, percentuale e barra. Se il server non sa quante righe leggerà, la barra scorre senza percentuale.

[[key:cancel]] smette di leggere e ferma la query anche sul server.

## Vista Dati
La vista **Dati** sfoglia tabelle, viste, viste materializzate e dizionari con filtri, ordinamento, ricerca, pagine e conteggio. È in sola lettura e lo dice in alto: «Sola lettura: ClickHouse non modifica le righe una per una.». Per cambiare i dati scrivi nell'editor `INSERT`, `ALTER TABLE … UPDATE` o `ALTER TABLE … DELETE`.

## Scritture e produzione
- `SELECT`, `SHOW`, `DESCRIBE`, `EXPLAIN` ed `EXISTS` sono letture; `SELECT … INTO OUTFILE` conta come scrittura.
- `INSERT`, `ALTER`, `CREATE`, `DROP`, `TRUNCATE`, `OPTIMIZE`, `KILL`, `SYSTEM` e ogni istruzione non riconosciuta passano dal controllo della produzione e finiscono nel registro ([Sicurezza in produzione](production)).
- `DROP`, `TRUNCATE` e `ALTER`, comprese le mutazioni, chiedono conferma come rischiose.
- Dopo un `INSERT` il messaggio dice quante righe sono state scritte.

**Genera SQL** scrive nella forma di ClickHouse: aggiornamento con `ALTER TABLE … UPDATE … WHERE`, cancellazione con `DELETE FROM … WHERE`, «upsert» come `INSERT` semplice (con `ReplacingMergeTree` vince l'ultima riga inserita per chiave), copia con `CREATE TABLE copia AS tabella` più `INSERT … SELECT`.

## Piano di esecuzione
**Spiega** disegna il piano come per gli altri motori ([Schema ER e piani di esecuzione](er-plans)). Per ogni indice usato si vede quante parti e quanti granuli restano: «parts 1/7 · granules 77/394». Il piano è sempre stimato: ClickHouse non lo misura eseguendo la query.

La pipeline si apre dalla palette dei comandi con «Apri la pipeline della query (EXPLAIN PIPELINE)»: una scheda nuova con `EXPLAIN PIPELINE` della query selezionata.

## Strumenti
- **Attività**: le query in corso, con utente, client, database, durata e testo. **Ferma la query** manda `KILL QUERY`; in produzione chiede lo sblocco.
- **Monitoraggio**: connessioni e query attive, le **Fusioni** in corso con le mutazioni, spazio per database, la tabella **Fusioni e mutazioni in corso** e le query lente delle ultime 24 ore raggruppate per forma; **Spiega** apre il piano dell'esempio.
- **Spazio occupato**: per tabella righe, dati compressi, chiave primaria in memoria e disco; sotto, le partizioni. Più di 100 parti in una partizione sono segnate: di solito vuol dire inserimenti troppo piccoli.
- **Importa ed esporta** come per gli altri motori ([Importare ed esportare](import-export)); le tabelle create dall'import usano tipi `Nullable(…)`.
- **Utenti e permessi**: utenti, ruoli e permessi; per l'utente o il ruolo scelto le **Quote**, i **Profili di impostazioni** (con minimi, massimi, `CONST` e profili ereditati) e le **Politiche di riga** con la loro condizione. **Nuova quota**, **Nuovo profilo** e **Nuova politica**, e la matita o il cestino su ogni riga, preparano lo SQL (`CREATE QUOTA`, `ALTER SETTINGS PROFILE`, `DROP ROW POLICY`…): lo correggi e lo esegui come dall'editor, con la conferma della produzione e il registro.

**Cerca un valore** e **Confronto dati** funzionano anche qui. Lo schema ER non c'è: ClickHouse non ha chiavi esterne.
- **Confronto schemi**: confronta il database scelto di due connessioni (la tendina in testata decide quale). Il copione crea le tabelle nuove con il `CREATE` del server, `ENGINE` compreso, aggiunge, cambia e toglie colonne e indici di salto con `ALTER TABLE`, ricrea viste e viste materializzate. La chiave primaria non cambia sul posto: la differenza ha la nota «da ricostruire». Motore, partizioni e dizionari non si confrontano.
- **Dati di prova**: senza transazioni, le righe vanno a blocchi da 1000. Se il server rifiuta un blocco il lavoro si ferma e dice quali righe e perché; i blocchi già scritti restano, con il conto per tabella.
- **Backup**: `BACKUP DATABASE … TO File('nome.zip')`, scritto dal server nella sua cartella dei backup; si sceglie solo il nome del file. Il ripristino si fa dall'editor con `RESTORE DATABASE … FROM File('nome.zip')`.
- **Sicurezza**: superutenti, utente `default`, password vuote o in chiaro, permessi di DDL, porta TLS, colonne sensibili, registro delle query, backup recenti, permessi `FILE`, `URL`, `S3` e simili.

## Limiti
- Solo HTTP e HTTPS: niente porta nativa né gRPC. Niente certificato client TLS.
- Niente transazioni dall'editor, modalità di commit e prova a secco.
- Una query interrotta dal server a metà (memoria, timeout) arriva come errore dopo le righe già ricevute, che restano nella griglia.
- Senza il permesso di leggere `system.processes` la query va senza barra di avanzamento.
- `USE altro_db` vale per la sola istruzione: per cambiare database usa la tendina dello schema.
- Le righe stimate del piano sono un tetto (granuli per 8192), non un conteggio.
