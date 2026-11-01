# Sicurezza in produzione

Una connessione con l'ambiente **produzione** si apre in sola lettura. Per scrivere si sblocca la scrittura, ogni scrittura chiede una conferma e finisce nel registro. Le regole si scelgono in **Impostazioni › Sicurezza**.

## Segnare una connessione come produzione
1. Tasto destro sulla connessione › **Modifica…**.
2. In **Ambiente** scegli **produzione**.
3. **Salva**.

Da quel momento:
- nella barra del titolo, accanto al nome della connessione, compare il bollo **produzione**;
- nella riga sotto le schede compare l'interruttore **Lettura** / **Scrittura**;
- vale il limite di tempo della produzione (vedi sotto).

Gli altri ambienti (**locale**, **sviluppo**, **staging**) non sono bloccati e cambiano solo il limite di tempo.

## Togliere una connessione dalla produzione
Cambiare l'ambiente da **produzione** a un altro spegne la protezione, anche nelle sessioni già aperte. Per questo il salvataggio chiede la stessa conferma di una scrittura, nella finestra «Togliere la connessione dalla produzione?»: con **Scrivere il nome** va scritto il nome che la connessione aveva prima. Il cambio finisce nel registro delle scritture («-- environment production -> local»).

Un'importazione che sostituisce una connessione di produzione la lascia in produzione.

## Sola lettura
Finché la scrittura è chiusa, l'interruttore dice **Sola lettura**.
- Le letture partono come sempre: `SELECT`, `SHOW`, `EXPLAIN`, i comandi di sola lettura di MongoDB e Redis.
- Una scrittura viene rifiutata con «Produzione in sola lettura: sblocca la scrittura per eseguire.».
- Dove il motore lo permette, la sessione è in sola lettura anche sul server: una scrittura nascosta in una funzione viene fermata dal server stesso. Succede su PostgreSQL e i compatibili, MySQL, MariaDB, SQLite, Firebird (transazioni di sola lettura), ClickHouse (`readonly=2` su ogni richiesta) e Oracle (le istruzioni che non sono query, fuori da una transazione aperta, girano in una transazione di sola lettura). SQL Server, DuckDB, SAP HANA, Db2, Cassandra, DynamoDB e OpenSearch si affidano al riconoscimento delle scritture di NiceDB.
- Vale per tutto quello che scrive: editor, vista Dati, editor della tabella, import, documenti di MongoDB, chiavi di Redis, «Ferma la query» in Strumenti › Attività.
- Strumenti › Dati di prova non scrive mai su una connessione di produzione.

## Sbloccare la scrittura
1. Nella riga sotto le schede, clic sull'interruttore.
2. Il riquadro dice per quanto resta aperta e quale conferma chiederà ogni scrittura.
3. **Sblocca la scrittura**.

- Con lo sblocco a tempo accanto all'interruttore scorre il tempo che resta: «ancora 9:42». A zero la scrittura si richiude da sola.
- Se il tempo scade mentre gira un'istruzione, l'istruzione finisce («tempo scaduto: si chiude a fine statement») e poi la scrittura si chiude.
- Un clic sull'interruttore aperto richiude subito (**Blocca ora**).
- [[Esc]] o un clic fuori chiude il riquadro senza sbloccare.
- Dopo una riconnessione la sessione riparte in sola lettura: lo sblocco va rifatto.
- Anche **Importa dati** su una connessione di produzione mostra l'interruttore nella sua finestra.

## Scegliere come si sblocca
**Impostazioni › Sicurezza › Sblocco della scrittura**:

| Modo | Cosa succede |
|---|---|
| **A tempo** | la scrittura resta aperta per la **Durata dello sblocco in produzione** (2, 5, 10, 30 o 60 minuti; 10 di serie), poi si richiude |
| **Fino alla disconnessione** | resta aperta finché non chiudi la connessione, senza conto alla rovescia |
| **Sempre aperta** | la produzione non è mai in sola lettura; al posto dell'interruttore compare «Scrittura sempre aperta» |

Il bollo **produzione** resta visibile in ogni modo, e le conferme restano quelle scelte sotto.

Passare a un modo più permissivo (qui o in **Conferma prima di scrivere**) chiede conferma nella finestra «Allentare la protezione della produzione?» e finisce nel registro di ogni connessione di produzione.

## Confermare una scrittura
**Impostazioni › Sicurezza › Conferma prima di scrivere**:

| Modo | Cosa chiede |
|---|---|
| **Scrivere il nome** (di serie) | per ogni scrittura scrivi il nome della connessione; il pulsante si attiva solo quando il nome coincide |
| **Finestra di conferma** | una finestra con **Annulla** e **Conferma**; il focus parte su **Annulla** |
| **Nessuna** | le scritture partono subito; resta solo il registro |

La finestra di conferma mostra:
- il titolo, per esempio «Eseguire in produzione?» o «Registrare 3 modifiche in produzione?», e il nome della connessione;
- le istruzioni o le modifiche che partiranno;
- per le istruzioni rischiose, le righe che toccheranno («tocca 1.240 righe»), quando si possono stimare;
- un avviso se nel frattempo qualcuno ha cambiato le righe che stai per modificare.

[[Esc]] equivale ad **Annulla**. Per provare una scrittura senza confermarla usa la prova a secco: vedi [Transazioni e prova a secco](transactions).

## Istruzioni rischiose
Chiedono conferma anche fuori dalla produzione, con la finestra «Eseguire statement rischiosi?» e il pulsante **Esegui comunque**:
- `UPDATE` o `DELETE` senza `WHERE`, `DROP`, `TRUNCATE`, `ALTER`;
- MongoDB: `delete` o `update` su tutti i documenti, `drop`, `dropDatabase`, `dropIndexes`, `collMod`, `renameCollection`;
- Redis: `FLUSHDB`, `FLUSHALL`, `KEYS`, `CONFIG SET`, `SHUTDOWN`, `CLIENT KILL` e simili;
- ClickHouse: `DROP`, `TRUNCATE`, `ALTER` comprese le mutazioni.

In produzione seguono il modo di conferma scelto: con **Nessuna** partono senza chiedere.

## Limiti di tempo
In **Impostazioni › Sicurezza** c'è un **Limite di tempo** per ogni ambiente. Una query che lo supera viene annullata sul server con «Interrotta dopo 60 s: limite di tempo dell'ambiente.».

| Ambiente | Di serie |
|---|---|
| locale | Nessun limite |
| sviluppo | Nessun limite |
| staging | 5 min |
| produzione | 1 min |

Valori possibili: Nessun limite, 15 s, 30 s, 1 min, 5 min, 15 min, 1 h.

## Registro delle scritture
Ogni scrittura, su qualsiasi connessione, viene annotata: quando, utente del sistema, database, righe toccate, testo inviato ed eventuale errore. Vale per editor, vista Dati (anche un filtro che scrive), documenti, chiavi, `COMMIT` di una transazione con scritture, sessioni fermate e ripristini. In produzione si annotano anche sblocco e blocco della scrittura, il cambio di ambiente e l'allentamento delle impostazioni di sicurezza.

Le password scritte nello SQL (`PASSWORD '…'`, `IDENTIFIED BY …`, `AUTH` di Redis, `pwd` di MongoDB e simili) diventano `***` sia nel registro sia nella cronologia.

1. Apri una tabella nella vista **Dati**.
2. Nella testata, **Registro**.
3. Per salvarlo: **Esporta** › **Tabella CSV** o **File JSON**.

Il pannello mostra le ultime 200 scritture della connessione; l'esportazione le contiene tutte.

## Limiti
- Con il commit manuale, sbloccare la scrittura chiude una transazione aperta che non ha ancora scritto: era nata in sola lettura.
- Il registro sta sul computer, nel database locale di NiceDB: non è un registro di controllo del server.
- L'utente annotato è quello del sistema operativo, non quello del database.
