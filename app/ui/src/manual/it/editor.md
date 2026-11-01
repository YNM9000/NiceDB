# Editor e completamento

L'editor parla il dialetto della connessione della scheda. Per MongoDB e Redis scrive i comandi del motore: vedi [MongoDB](mongodb) e [Redis e Valkey](redis).

## Tasti dell'editor
| Tasto | Effetto |
|---|---|
| [[key:run_statement]] | esegue l'istruzione sotto il cursore, o la selezione |
| [[key:run_script]] | esegue tutto il testo, o la selezione |
| [[key:cancel]] | ferma la query in corso |
| [[key:format]] | formatta la selezione, o tutto il testo |
| [[Ctrl+/]] | commenta o scommenta le righe |
| [[Ctrl+Spazio]] | apre il completamento |
| [[Ctrl+F]] | trova e sostituisci; [[F3]] e [[Shift+F3]] vanno alla corrispondenza successiva e precedente |
| [[Ctrl+Alt+G]] | vai alla riga |
| [[Alt+↑]] / [[Alt+↓]] | sposta la riga in su o in giù |
| [[Tab]] / [[Shift+Tab]] | rientra e toglie il rientro |
| [[key:rename_alias]] | rinomina l'alias sotto il cursore |
| [[Ctrl+Z]] / [[Ctrl+Shift+Z]] | annulla e ripete nel testo |

Le scorciatoie con il nome dell'azione si cambiano in **Impostazioni › Scorciatoie da tastiera**. Il tasto destro nell'editor offre le stesse azioni, più **Esegui lo script senza fermarti agli errori** e **Prova a secco**.

## Completamento
Si apre da solo mentre scrivi un nome, dopo un punto, dopo `JOIN` e dopo `ON`; [[Ctrl+Spazio]] lo apre ovunque. [[Invio]] accetta, [[Esc]] chiude. Le proposte vengono dallo schema già caricato: mentre scrivi non parte nessuna query verso il server.

| Dove | Cosa propone |
|---|---|
| dopo `FROM`, `JOIN`, `INTO`, `UPDATE`, `DELETE FROM` | tabelle, viste, le tabelle della `WITH`, gli schemi |
| dopo `JOIN` | prima i join completi presi dalle chiavi esterne |
| dopo `ON` | la condizione di join dalle chiavi esterne |
| dopo `alias.` o `tabella.` | le colonne di quella tabella, con il tipo |
| dopo `schema.` | tabelle, viste e funzioni dello schema |
| in `SELECT`, `WHERE`, `GROUP BY`, `ORDER BY`, `SET` | colonne delle tabelle della query, alias, funzioni, parole chiave |
| dentro `INSERT INTO t (…)` | le colonne di `t` |
| dopo `::` e in `CAST(… AS` | i tipi del motore |
| dopo `USE` | i database del server |
| a inizio istruzione | `SELECT`, `INSERT INTO`… e gli snippet |

```sql
select * from clienti c join |        -- fatture f on f.cliente_id = c.id
select r.| from (select id as cliente, citta from clienti) r   -- cliente, citta
```

- Alias di tabelle, sottoquery e `WITH` sono riconosciuti; una sottoquery correlata vede anche le tabelle esterne.
- Una colonna presente in due tabelle della query arriva qualificata (`o.id`, `c.id`).
- Le tabelle dello schema predefinito entrano col solo nome, le altre con lo schema davanti. Su PostgreSQL e CockroachDB lo schema predefinito è il `search_path` della sessione: lo legge all'apertura e di nuovo dopo un `SET search_path` eseguito nella scheda; una tabella fuori dal percorso entra con lo schema.
- Le virgolette si aggiungono solo quando servono, nello stile del dialetto: `"Ordini Web"`, `` `Ordini Web` ``, `[Ordini Web]`.
- Una funzione entra con le parentesi e il cursore in mezzo.
- Le parole chiave seguono le maiuscole o minuscole dell'istruzione.
- Le scelte frequenti e recenti salgono in cima.

A sinistra di ogni proposta un segno ne dice il tipo: **T** tabella, **V** vista, **W** tabella della `WITH`, **C** colonna, **ƒ** funzione, **⋈** join dalla chiave esterna, **S** schema, **D** database, **A** alias, **τ** tipo, **§** snippet.

## Scheda della colonna
Scegliendo una colonna nell'elenco, accanto compare la sua scheda: tipo, NULL ammesso o no, predefinito, commento, numerazione automatica, la chiave esterna a cui punta, chi punta a lei e gli indici che la contengono. La stessa scheda si apre fermando il mouse su una colonna nel testo (anche dietro un alias, `f.totale`) e, nell'esploratore, con un clic o [[Invio]] su una colonna o dal tasto destro › **Informazioni sulla colonna**. Predefinito e commento si leggono dal catalogo del server la prima volta che servono.

## Avvisi sui nomi
Una tabella che lo schema caricato non conosce, o una colonna che la tabella non ha, è sottolineata a puntini. Il suggerimento dice «Tabella sconosciuta: …» o «… non ha la colonna …»; su PostgreSQL, per una tabella che esiste ma fuori dal `search_path`, «… non è nel search_path: sta in …». L'avviso non ferma l'esecuzione e tace dove non può sapere: istruzioni DDL, tabelle create dallo script stesso, tabelle temporanee, schemi di sistema, funzioni che fanno da tabella.

Si spengono in **Impostazioni › Editor › Nomi sconosciuti sottolineati**, o dalla palette.

## Rinominare un alias
1. Metti il cursore su un alias, dove è definito o dove è usato.
2. Premi [[key:rename_alias]], o tasto destro › **Rinomina l'alias**.
3. Scrivi: tutte le occorrenze nell'istruzione cambiano insieme.

Vale anche per i nomi della `WITH`, non per tabelle e colonne vere.

## Cursori multipli
| Tasto | Effetto |
|---|---|
| [[Ctrl+D]] | seleziona la prossima occorrenza |
| [[Ctrl+Shift+L]] | seleziona tutte le occorrenze |
| [[Ctrl+Alt+↑]] / [[Ctrl+Alt+↓]] | aggiunge un cursore sopra o sotto |
| [[Ctrl]] + clic | aggiunge un cursore |
| [[Alt]] + trascina | selezione rettangolare |
| [[Esc]] | torna a un cursore solo |

Esegui e Formatta lavorano sul cursore principale.

## Script con più istruzioni
- Le istruzioni si separano con `;`. Su SQL Server una riga con solo `GO` chiude il batch; `GO 5` lo esegue cinque volte.
- Su MySQL e MariaDB vale il `DELIMITER` del client mysql: dopo `DELIMITER $$` le istruzioni finiscono a `$$` e il corpo di una procedura resta intero; `DELIMITER ;` torna al punto e virgola. Le righe `DELIMITER` e i delimitatori sono colorati a parte.
- Uno script si ferma alla prima istruzione che fallisce. **Esegui lo script senza fermarti agli errori** (tasto destro o palette) va fino in fondo; per farlo sempre: **Impostazioni › Editor › Errore in uno script › Continua**.
- Si ferma comunque quando l'errore ha rovinato la transazione aperta (PostgreSQL, CockroachDB, DuckDB).

Durante l'esecuzione l'istruzione in corso è evidenziata; dopo un errore, il punto indicato dal server è segnato nel testo.

## Impostazioni dell'editor
In **Impostazioni › Editor**: **Dimensione del carattere**, **Larghezza del rientro**, **Autocompletamento**, **Parole chiave maiuscole in Formatta**, **Nomi sconosciuti sottolineati**, **Errore in uno script**. Vedi [Impostazioni](settings).

## Limiti
- Lo schema è quello caricato: una tabella creata da un altro client compare dopo l'aggiornamento dello schema.
- Un `search_path` cambiato da una funzione o da un'altra connessione si vede alla prossima esecuzione che lo nomina, o riaprendo lo schema.
- Le colonne di una funzione che restituisce una tabella sono note solo se le nomini con l'alias, `x(a, b)`.
- Il `DELIMITER` deve stare all'inizio di un'istruzione, su una riga sua, come nel client mysql.
