# Oracle

Oracle si usa come gli altri motori SQL, con il suo dialetto e PL/SQL. NiceDB parla il protocollo di Oracle da solo: non serve installare Instant Client né altro.

## Connettersi
1. **Nuova connessione**, poi in **Motore** scegli Oracle.
2. **Indirizzo**:
   - **Host e servizio**: host, porta (1521) e **Servizio**, per esempio `FREEPDB1` o `ORCLPDB1`. Se il database si raggiunge per SID, spunta **Il nome è un SID, non un servizio**.
   - **Stringa di connessione**: una stringa Easy Connect (`db.bassi.lan:1521/GESTIONALE`) o un descrittore intero `(DESCRIPTION=…)`, come in `tnsnames.ora`. Con la stringa il tunnel SSH e il proxy non si usano.
3. **Utente** e **password**. La password sta nel portachiavi.
4. **Privilegio di accesso**: **Normale**, **SYSDBA** o **SYSOPER**.
5. **TLS**: con TLS NiceDB usa TCPS, di solito sulla porta 2484. Il **Certificato CA** diventa l'unica radice fidata; **Cifrato, senza verifica** salta il controllo del nome, ma la catena del certificato si verifica comunque.
6. **Prova**, poi **Salva**.

**Copia come URL** dal tasto destro sulla connessione dà `oracle://utente@host:1521/servizio`; lo stesso URL incollato nel form lo compila.

Non ci sono Kerberos, wallet con password memorizzata, autenticazione esterna e la cifratura nativa di rete (NNE): per cifrare serve TLS.

## Sessione
Ogni sessione parte con date e numeri in un formato che si rilegge senza ambiguità: `YYYY-MM-DD HH24:MI:SS` per le date, `.` come separatore decimale. Le query di init della connessione girano dopo, per esempio:

```sql
ALTER SESSION SET CURRENT_SCHEMA = GESTIONALE;
ALTER SESSION SET TIME_ZONE = 'Europe/Rome';
```

## Esplorare
Sotto la connessione ci sono gli schemi: quello dell'utente per primo, poi quelli che hanno oggetti e non sono di Oracle (`SYS`, `SYSTEM`, `XDB` e gli altri restano fuori). In ogni schema:
- **Tabelle**, con le righe stimate dalle statistiche; aprendole, colonne con il tipo (`NUMBER(12,2)`, `VARCHAR2(80 CHAR)`…), chiave primaria e indici.
- **Viste** e **viste materializzate**.
- **Funzioni** e procedure con i parametri, **Pacchetti**, **Sequenze**, **Sinonimi** con l'oggetto a cui puntano, **Tipi** e **Trigger** con la loro tabella.

Tasto destro › **Mostra DDL** chiede il testo a `DBMS_METADATA.GET_DDL`: per una tabella anche gli indici e i commenti, per un pacchetto specifica e corpo.

## L'editor
- Le istruzioni SQL si separano con `;`, come altrove; NiceDB toglie il `;` prima di mandarle, perché Oracle non lo vuole.
- I blocchi PL/SQL (`BEGIN … END;`, `DECLARE …`) e le `CREATE` di procedure, funzioni, pacchetti, tipi e trigger restano interi con il loro `END;`. Una riga con solo `/` chiude un blocco, come in SQL*Plus; dopo un tipo o un trigger composto serve.
- `EXEC procedura(1)` diventa `BEGIN procedura(1); END;`.
- Le stringhe `q'[…]'` sono riconosciute, i segnaposto `:nome` si compilano come nelle altre query; `:new` e `:old` nei trigger non sono segnaposto.
- La macro `block` scrive un blocco anonimo.

## Transazioni
Oracle non conferma da solo sul protocollo: NiceDB lo fa dopo ogni istruzione finché la scheda è in commit automatico. In commit manuale o intelligente la transazione parte con `SET TRANSACTION READ WRITE` e aspetta **Commit** o **Rollback**. Ogni istruzione DDL (`CREATE`, `ALTER`, `DROP`, `TRUNCATE`, `GRANT`…) conferma la transazione aperta: la scheda lo dice.

Attenzione: in Oracle una stringa vuota è `NULL`. Svuotare una cella di testo nella vista Dati scrive `NULL`.

## Vista Dati
Le righe si modificano per chiave primaria o per un indice unico. Una tabella che non ne ha usa il `ROWID`: compare come ultima colonna e serve solo a ritrovare la riga. Le celle `CLOB`, `BLOB` e `JSON` si modificano, ma non servono a riconoscere una riga cambiata da altri; `RAW` si scrive in esadecimale (`\xCAFE`).

## Piano di esecuzione
**Spiega** esegue l'istruzione con `STATISTICS_LEVEL = ALL` e legge `DBMS_XPLAN.DISPLAY_CURSOR` con `ALLSTATS LAST`: righe vere, tempi e letture per passo. Con **Solo stima** usa `EXPLAIN PLAN` e legge la tabella di `DBMS_XPLAN.DISPLAY`, senza eseguire niente. Per i tempi veri serve poter leggere `V$SESSION`, `V$SQL` e `V$SQL_PLAN_STATISTICS_ALL` (per esempio con `SELECT_CATALOG_ROLE`); senza, il piano resta una stima e lo dice. Una scrittura misurata si annulla alla fine, come sugli altri motori.

## Fermare una query
- Con il privilegio `ALTER SYSTEM`, **Annulla** chiude la sessione sul server se la query non si ferma entro un secondo e mezzo; NiceDB ne apre subito un'altra e lo dice. Il driver di Oracle in Rust non sa ancora riprendersi dopo un `ALTER SYSTEM CANCEL SQL`, quindi chiudere è l'unica strada pulita. Con una transazione aperta la sessione non si chiude.
- Senza il privilegio, **Annulla** ferma le righe al blocco successivo e il suggerimento sul pulsante lo spiega; il lavoro già partito sul server continua finché finisce.

## Strumenti
- **Attività**: le sessioni di `V$SESSION` con istruzione, attesa e chi blocca chi. Il codice di una sessione è `sid,serial#`. **Ferma la query** usa `ALTER SYSTEM CANCEL SQL`, **Chiudi la sessione** `ALTER SYSTEM KILL SESSION … IMMEDIATE`, sempre dal varco della produzione.
- **Monitoraggio**: sessioni, transazioni aperte, dimensione dei file dati, rapporto di lettura dalla cache, **Operazioni lunghe** da `V$SESSION_LONGOPS` e le istruzioni più costose da `V$SQLAREA`. Senza `SELECT_CATALOG_ROLE` alcune parti dicono che cosa manca.
- **Spazio**: tabelle e indici da `DBA_SEGMENTS`, o solo il proprio schema da `USER_SEGMENTS`.
- **Utenti e ruoli**: utenti, ruoli e privilegi; crea, assegna e revoca con l'anteprima dello SQL. Un utente nuovo nasce senza password (`NO AUTHENTICATION`): gliela dai con `ALTER USER … IDENTIFIED BY …`.
- **Manutenzione** (tasto destro su tabella o schema): **Aggiorna le statistiche** con `DBMS_STATS`.
- Il backup non si fa da qui: usa Data Pump (`expdp`, `impdp`) o RMAN sul server.
