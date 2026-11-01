# DDL e generatori SQL

Dal tasto destro su un oggetto, nell'esploratore o su un riquadro dello schema ER, si legge la sua definizione presa dal server, si scrivono le istruzioni più comuni su una tabella e si vede cosa dipende da cosa.

## Leggere il DDL
1. Tasto destro su una tabella, vista, funzione, procedura, trigger, indice o sequenza.
2. **Mostra DDL**.
3. Si apre una finestra con l'istruzione di creazione evidenziata, in sola lettura: «Generato da PostgreSQL, in sola lettura.»
4. **Copia** la mette negli appunti, **Apri nell'editor** la scrive in una nuova scheda di query, **Chiudi** chiude la finestra.

**Copia CREATE**, nello stesso menu, mette il DDL negli appunti senza aprire la finestra. **Mostra DDL** sta anche nella palette: [[key:palette]], cercare l'oggetto, [[→]] per le sue azioni.

Il DDL è quello che scrive il motore, con valori predefiniti, vincoli, identità e commenti:

| Motore | Da dove viene |
|---|---|
| PostgreSQL, CockroachDB | ricostruzione dal catalogo, `pg_get_viewdef`, `pg_get_functiondef`, `pg_get_triggerdef`, `pg_get_indexdef` |
| MySQL, MariaDB | `SHOW CREATE TABLE`, `VIEW`, `FUNCTION`, `PROCEDURE`, `TRIGGER` |
| SQL Server | `sys.sql_modules` e ricostruzione delle tabelle |
| SQLite, libSQL | il testo salvato in `sqlite_schema` |
| DuckDB | `duckdb_tables()`, `duckdb_views()`, `duckdb_functions()` |

## Generare SQL
Tasto destro su una tabella o una vista › **Genera SQL**. La voce scelta scrive l'istruzione in una **nuova scheda di query** sullo stesso database, senza eseguirla. **Copia negli appunti**, in fondo al sottomenu, ha le stesse voci e mette l'istruzione negli appunti.

| Voce | Cosa scrive |
|---|---|
| SELECT | tutte le colonne per nome, una per riga |
| INSERT | le colonne con un segnaposto per ciascuna, tranne quelle che il server numera da sé (identità, `serial`, `AUTO_INCREMENT`) |
| **UPDATE per chiave** | le colonne fuori dalla chiave in `SET`, la chiave in `WHERE` |
| **DELETE per chiave** | `WHERE` sulla chiave |
| INSERT … ON CONFLICT, INSERT … ON DUPLICATE KEY, MERGE | l'upsert: inserisce la riga o, se la chiave c'è già, la aggiorna; il nome della voce segue il motore |
| **Copia della tabella** | una tabella `<nome>_copia` con le stesse colonne e le stesse righe |
| DROP TABLE, DROP VIEW | l'eliminazione dell'oggetto |

```sql
UPDATE fatture
SET
  data = :data,
  totale = :totale,
  pagata = :pagata
WHERE numero = :numero;
```

- I valori sono segnaposto `:colonna`: alla prima esecuzione compare la barra dei parametri (vedi [Parametri nelle query](parameters)).
- La chiave è la chiave primaria; senza, vale un indice unico su colonne NOT NULL. Il suggerimento accanto alla voce dice quale («per numero»). Senza nessuna delle due, UPDATE, DELETE e upsert restano spenti con «senza chiave primaria».
- I nomi prendono le virgolette del motore solo quando servono (`"Nome"`, `` `order` ``, `[user]`); lo schema compare solo se non è quello predefinito.
- Sulle viste ci sono solo SELECT, **Tabella dalla vista** e DROP VIEW.
- Un DROP si apre nell'editor come le altre voci; eseguendolo, l'analisi di rischio chiede conferma e in produzione vale lo sblocco.

Upsert e copia per motore:

| Motore | Upsert | Copia della tabella |
|---|---|---|
| PostgreSQL, CockroachDB | `ON CONFLICT (chiave) DO UPDATE SET c = EXCLUDED.c` | `CREATE TABLE … (LIKE … INCLUDING ALL)` più `INSERT … SELECT` |
| MySQL, MariaDB | `ON DUPLICATE KEY UPDATE c = VALUES(c)` | `CREATE TABLE … LIKE …` più `INSERT … SELECT` |
| SQL Server | `MERGE … USING (VALUES …) AS s`, fra `SET IDENTITY_INSERT … ON` e `OFF` se la tabella ha un'identità | `SELECT * INTO … FROM …` più la chiave primaria |
| SQLite, libSQL, DuckDB | `ON CONFLICT (chiave) DO UPDATE SET c = EXCLUDED.c` | `CREATE TABLE …` con colonne, tipi, `NOT NULL` e chiave primaria, più `INSERT … SELECT` |

Con la tabella aperta nella vista **Dati**, la palette dei comandi ([[key:palette_commands]]) ha «Genera la SELECT della tabella», «Genera l'INSERT della tabella», «Genera l'UPDATE per chiave», «Genera il DELETE per chiave», «Genera l'upsert della tabella» e «Mostra le dipendenze della tabella».

## Vedere le dipendenze
1. Tasto destro su una tabella o una vista › **Dipendenze…**
2. La finestra divide gli oggetti legati in due gruppi: **Usa** (per una vista **Legge da**) e **La usano**.
3. Clic su una tabella o una vista dell'elenco per vedere le sue dipendenze; **Indietro** torna alla precedente.
4. A destra di ogni riga, **Apri i dati** e **Mostra DDL**.

| Legame | Significato |
|---|---|
| chiave esterna `nome` | la tabella punta all'altra, o l'altra punta a questa |
| nella definizione | una vista, una funzione o un trigger nomina l'oggetto nel suo testo |
| trigger · `funzione` | un trigger sulla tabella e la funzione che esegue |
| sequenza della tabella | una sequenza che riempie una colonna (`serial`, identità) |

Su PostgreSQL, CockroachDB e SQL Server le dipendenze vengono dal catalogo del server. Su MySQL, SQLite e DuckDB le viste le legge il motore stesso; routine, macro e corpi dei trigger (e le viste su MariaDB e libSQL remoto) si cercano nel testo delle definizioni, e la finestra lo dice in basso.

## Limiti
- L'INSERT elenca anche le colonne con un valore predefinito o un'identità: su SQL Server una colonna `IDENTITY` va tolta a mano.
- **Copia della tabella** copia struttura e righe; su SQLite, DuckDB, SQL Server e dalle viste non porta chiavi, indici e vincoli. Non controlla che `<nome>_copia` non esista già.
- Su PostgreSQL una funzione compare tra le dipendenze di una vista che la usa, non tra quelle delle tabelle che legge.
- La ricerca nel testo conta anche un nome citato in un commento o in una stringa, e non distingue due tabelle omonime in schemi diversi.
- Le dipendenze riguardano tabelle e viste; per funzioni, trigger e sequenze c'è **Mostra DDL**.
