# Esploratore

L'esploratore, nella barra a sinistra, mostra sotto ogni connessione aperta i suoi database e i loro oggetti in un albero unico. Da qui si aprono i dati, si scrivono le prime query e si arriva agli strumenti con il tasto destro.

## L'albero
Una connessione relazionale si espande così:

- **Database** › database › **Schemi** › schema › **Tabelle**, **Viste**, **Funzioni**, **Sequenze**, **Tipi**, **Trigger**
- **Utenti e ruoli** e **Sessioni attive**, a livello del server

Su MySQL, MariaDB, ClickHouse e CockroachDB i database non ancora aperti mostrano quante tabelle hanno, letto dal catalogo del server con una sola query.

Su PostgreSQL e SQL Server un database con almeno 2000 tabelle e viste in più schemi si apre a livelli: subito l'elenco degli schemi con quante tabelle e viste ha ognuno, e gli oggetti del primo schema; gli altri si leggono quando li si espande. Finché uno schema non è aperto, l'autocompletamento, la palette e lo schema ER non ne vedono le tabelle.

Compaiono solo i livelli che il motore ha davvero: SQLite, libSQL e DuckDB sono un file e vanno subito agli schemi o alle tabelle, MySQL e MariaDB non hanno schemi dentro il database. ClickHouse aggiunge **Viste materializzate** e **Dizionari**, MongoDB mostra **Collezioni** e **Campi**, Redis e Valkey i **Prefissi** delle chiavi (vedi [MongoDB](mongodb), [Redis e Valkey](redis), [ClickHouse](clickhouse)).

- Accanto a una tabella c'è il numero stimato di righe (`~12.400`).
- Il database della scheda attiva ha un puntino.
- Espandendo una tabella si vedono colonne e indici.
- Con migliaia di tabelle l'albero scorre senza rallentare: disegna solo le righe visibili.

## Aprire oggetti
| Azione | Effetto |
|---|---|
| Clic su una tabella o vista | apre i dati nella vista **Dati** (vedi [Vista Dati e modifiche](data)) |
| Doppio clic su una tabella o vista | apre una nuova scheda con la `SELECT` della tabella |
| Clic su un database | apre una scheda su quel database |
| Clic sul triangolo | espande o chiude il nodo |

Da tastiera, con il focus nell'albero:

| Tasto | Effetto |
|---|---|
| [[↑]] [[↓]], [[Home]], [[Fine]] | sposta la selezione |
| [[→]] | espande il nodo, o scende al primo figlio |
| [[←]] | chiude il nodo, o sale al padre |
| [[Invio]] | come il clic |
| [[Spazio]] | espande o chiude |
| lettere | salta al primo nodo che comincia con quel testo |

## Filtrare
- **Filtra oggetti**, sopra l'albero, restringe gli oggetti mentre si scrive. [[Invio]] nel campo apre la palette su **Oggetti** con lo stesso testo, per cercare in tutti i database aperti (vedi [Palette dei comandi e scorciatoie](palette)).
- **Cerca connessioni**, più in alto, filtra le connessioni; [[Invio]] apre la palette su **Libreria**.

## Cambiare database e nascondere quelli inutili
La barra sopra il filtro mostra il database della scheda attiva.
- Clic sul nome (**Cambia database**): l'elenco dei database del server; «in uso» segna quelli aperti in altre schede.
- L'icona delle impostazioni (**Database e schemi visibili**) apre le caselle dei database e degli schemi da mostrare, con **Cerca**, **Solo quelli usati** (tiene solo quelli che le schede usano) e **Predefiniti**.
- Tasto destro su un database › **Nascondi il database**, su uno schema › **Nascondi lo schema**. [[Ctrl+Z]] li fa ricomparire.
- Con oggetti nascosti, sotto la barra compare **Mostra tutti** con il conteggio («3 nascosti»); **Nascondi di nuovo** torna alla scelta.
- **Aggiorna schema** (la freccia circolare) rilegge lo schema dal server, per esempio dopo un `CREATE TABLE` fatto da un altro programma.

## Menu del tasto destro
Ogni nodo ha il suo menu. Le voci degli strumenti compaiono solo se il motore li ha (vedi [Strumenti](tools)).

| Nodo | Voci principali |
|---|---|
| Tabella | **Apri i dati**, **Nuova query SELECT**, **Mostra nello schema ER**, **Mostra DDL**, **Modifica tabella…**, **Genera SQL**, **Dipendenze…**, **Copia nome**, **Copia SELECT**, **Copia CREATE**, **Esporta dati…**, **Importa dati…**, **Dati di prova…**, la manutenzione del motore, **Mostra colonne** |
| Vista | come la tabella, senza modifica, importazione, dati di prova e manutenzione |
| Database | **Apri una scheda su questo database**, **Nuova query qui**, **Cerca un valore…**, **Confronto schemi**, **Backup e ripristino**, **Sicurezza**, **Spazio occupato**, **Monitoraggio**, la manutenzione del database, **Copia nome**, **Nascondi il database** |
| Schema | **Nuova query qui**, **Nuova tabella…**, **Cerca un valore…**, **Copia nome**, **Nascondi lo schema** |
| Gruppo Tabelle | **Nuova tabella…** |
| Funzione, procedura | **Mostra DDL**, **Copia CREATE**, **Copia firma**, **Copia nome** |
| Trigger, sequenza, tipo, indice | **Mostra DDL**, **Copia CREATE**, **Copia nome** |
| Colonna | **Copia nome**, **Copia nome completo** |
| Utente o ruolo | **Permessi…** (anche con il clic sul nodo: si apre sull'utente), **Copia nome** |
| Sessione attiva | **Ferma la query**, **Chiudi la sessione**, **Copia la query**, **Apri l'attività del server**, **Monitoraggio** |

Quasi ogni menu termina con **Aggiorna schema**. DDL, generatori e dipendenze sono spiegati in [DDL e generatori SQL](ddl), la modifica della struttura in [Editor visuale della tabella](designer).

## Manutenzione
Su tabelle e database il menu propone la manutenzione che il motore offre; ogni voce mostra a destra l'istruzione che userà.

| Motore | Tabella | Database |
|---|---|---|
| PostgreSQL | **Ripulisci** (`VACUUM (ANALYZE)`), **Aggiorna le statistiche** (`ANALYZE`) | le stesse su tutto il database |
| MySQL, MariaDB | **Ricostruisci la tabella** (`OPTIMIZE TABLE`), **Aggiorna le statistiche** (`ANALYZE TABLE`) | nessuna |
| SQL Server | **Aggiorna le statistiche** (`UPDATE STATISTICS`) | `sp_updatestats` |
| CockroachDB | **Aggiorna le statistiche** (`ANALYZE`) | nessuna |
| SQLite, libSQL | **Aggiorna le statistiche** (`ANALYZE`) | **Ripulisci** (`VACUUM`), **Aggiorna le statistiche** |
| DuckDB | **Aggiorna le statistiche** (`ANALYZE`) | **Aggiorna le statistiche**, **Ripulisci** (`CHECKPOINT`) |

1. Scegliere la voce.
2. Confermare l'istruzione. In produzione servono lo sblocco e la conferma, come per ogni scrittura (vedi [Sicurezza in produzione](production)).
3. Il lavoro gira su una connessione a parte, così l'editor resta libero; alla fine un avviso dice quanto è durato. L'operazione finisce nel registro delle scritture.

## Limiti
- L'esploratore lavora sullo schema letto all'apertura: gli oggetti creati da altri programmi compaiono dopo **Aggiorna schema**.
- Ogni database espanso tiene aperta una sessione finché il nodo resta aperto.
