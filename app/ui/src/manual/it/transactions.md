# Transazioni e prova a secco

Funziona su PostgreSQL, CockroachDB, MySQL, MariaDB, SQL Server, SQLite, libSQL e DuckDB.

## Modo di commit
Il pulsante a destra della riga delle schede mostra il modo della sessione della scheda attiva (**Auto**, **Manuale**, **Smart**). Un clic apre il menu:

| Modo | Comportamento |
|---|---|
| **Commit automatico** | ogni istruzione si conferma da sola (predefinito) |
| **Commit manuale** | prima della prima istruzione si apre una transazione; tutto aspetta **Commit** o **Rollback** |
| **Commit intelligente** | automatico finché leggi; la prima scrittura apre la transazione, che resta fino a Commit o Rollback |

- Il modo vale per la sessione: le schede della stessa connessione e dello stesso database lo condividono.
- Resta dopo una riconnessione. Cambiarlo non chiude una transazione già aperta.
- Un `BEGIN` scritto nell'editor apre la transazione anche in automatico; `COMMIT` e `ROLLBACK` scritti a mano la chiudono, e la barra li segue.

## La transazione aperta
Con una transazione aperta, accanto al modo compare **Transazione aperta** con scritture, righe e durata, per esempio «3 scritture · 128 righe · 1:24», e i pulsanti **Rollback** e **Commit**. Si vede in tutte le viste.

- **Rollback** è sempre permesso, anche con la produzione bloccata.
- **Commit** su produzione chiede lo sblocco della scrittura: vedi [Sicurezza in produzione](production).
- Su PostgreSQL, CockroachDB e DuckDB un errore rende la transazione inutilizzabile: l'indicatore dice «in errore: solo Rollback» e **Commit** è spento.
- MySQL e MariaDB confermano da soli la transazione prima di un'istruzione DDL (`CREATE`, `ALTER`, `DROP`, `TRUNCATE`…): la scheda lo dice e l'indicatore si spegne.
- Istruzioni che il motore non accetta in una transazione (`VACUUM`, `CREATE DATABASE`, `CREATE INDEX CONCURRENTLY`, `ATTACH`…) girano fuori, senza aprirne una.

Nella vista Dati, con commit manuale o intelligente o con una transazione aperta, **Applica** non conferma: le modifiche entrano nella transazione e aspettano **Commit**. Vedi [Vista Dati e modifiche](data).

## Prova a secco
La prova a secco esegue una scrittura in una transazione, mostra le righe toccate e aspetta la tua decisione.

1. Metti il cursore sull'istruzione, o seleziona più istruzioni.
2. Premi **Prova a secco** nella barra della query o [[key:dry_run]], oppure tasto destro nell'editor › **Prova a secco**.
3. Guarda la scheda **Prima e dopo**: nelle modifiche il valore vecchio è barrato e quello nuovo accanto, nelle cancellazioni le righe sono barrate, negli inserimenti si vedono le righe nuove.
4. Sopra i risultati: «Prova a secco: 1 scrittura · 300 righe. Niente è confermato finché non premi Commit».
5. Premi **Commit** per tenere le modifiche, **Rollback** per buttarle.

![Prova a secco di una scrittura](anim:dry-run)

- Si vedono le prime 200 righe per istruzione; il conteggio è sempre quello vero.
- Per un `UPDATE` i valori nuovi vengono da una prova dell'istruzione in un savepoint, annullata subito dopo: si vedono `now()`, i valori predefiniti e le colonne cambiate dai trigger. Dove non si può (DuckDB, libSQL remoto, tabelle senza chiave primaria o MyISAM) sono una stima e la scheda lo scrive sotto la tabella.
- Gli `INSERT` mostrano le righe inserite su PostgreSQL, CockroachDB, SQLite, libSQL e DuckDB; su MySQL, MariaDB e SQL Server solo il conteggio.
- Se un'istruzione fallisce, o la fermi con **Annulla**, la prova si annulla da sola.
- La prova non chiede conferma per le istruzioni rischiose: le righe le vedi prima di confermare.
- Su produzione la scrittura va sbloccata già per la prova, perché le righe vengono davvero toccate e bloccate fino alla chiusura; il nome della connessione si chiede al **Commit**.

Non si possono provare a secco istruzioni di transazione (`BEGIN`, `COMMIT`…), istruzioni che il motore rifiuta in una transazione e, su MySQL e MariaDB, il DDL. Serve anche che non ci sia già una transazione aperta. In questi casi la prova non parte e la scheda dice quale istruzione la blocca.

## Chiudere con una transazione aperta
Chiudere l'ultima scheda di una sessione, disconnettere, cambiare spazio di lavoro o uscire con una transazione che contiene scritture apre una finestra:

| Pulsante | Effetto |
|---|---|
| **Non chiudere** | lascia tutto com'è |
| **Annulla e chiudi** | rollback, poi chiude |
| **Conferma e chiudi** | commit, poi chiude; su produzione passa dallo sblocco |

Una transazione senza scritture si chiude senza chiedere.

## Connessione caduta e inattività
La transazione vive nella connessione: se la connessione cade, il server la annulla. La scheda lo dice sempre:
- caduta durante una query: «La transazione aperta è stata annullata dal server»;
- connessione ripristinata dal keep-alive o da **Riconnetti**: l'avviso dice che la transazione è stata annullata;
- sessione riaperta al primo uso: la transazione è andata persa con le sue scritture;
- chiusura per inattività: la transazione viene annullata e l'avviso lo dice.

Il modo di commit resta quello di prima. Le opzioni di sessione sono in [Connessioni e profili](connections).

## Ctrl+Z
[[Ctrl+Z]] non tocca mai le transazioni: non annulla un Commit né un Rollback. Riaprire con [[Ctrl+Z]] una scheda chiusa non riporta la transazione chiusa con lei.

## Limiti
- L'anteprima di un `UPDATE` calcola i valori nuovi con una `SELECT` prima di eseguire: `now()` o `random()` possono dare valori un po' diversi da quelli scritti, e i trigger non si vedono.
- Per `UPDATE … FROM`, join, `MERGE`, assegnazioni a gruppi, `ORDER BY`/`LIMIT` nelle scritture e `UPDATE TOP` si vede solo il conteggio.
- L'anteprima ha 5 secondi; se non bastano, la prova va avanti senza.
- SQL Server può annullare da solo una transazione dopo certi errori gravi; l'indicatore lo scopre al Commit. Un `BEGIN` dentro un batch o una procedura di SQL Server non viene visto dalla barra.
- DuckDB non ha savepoint: con una transazione aperta, **Applica** nella vista Dati chiede di chiuderla prima.
- Su produzione con commit manuale, sbloccare la scrittura chiude una transazione aperta ancora senza scritture.
