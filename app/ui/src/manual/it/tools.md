# Strumenti

La vista **Strumenti** raccoglie il lavoro sul server: sessioni, carico, spazio, permessi, confronti, ricerche, dati di prova, backup, import ed export, controllo di sicurezza. Si apre dal selettore delle viste, dal tasto destro su un database nell'esploratore o dalla palette dei comandi ([[key:palette_commands]]), per esempio «Apri Monitoraggio».

A sinistra c'è l'elenco degli strumenti; uno strumento che il motore non ha resta in elenco con «Non disponibile per questo motore». Ogni strumento lavora sul database aperto nella scheda.

## Quali motori li hanno
| Strumento | Motori |
|---|---|
| **Server** | Redis, Valkey |
| **Attività** | PostgreSQL, CockroachDB, MySQL, MariaDB, SQL Server, ClickHouse |
| **Monitoraggio**, **Spazio occupato**, **Importa ed esporta** | gli otto relazionali e ClickHouse |
| **Utenti e permessi** | PostgreSQL, CockroachDB, MySQL, MariaDB, SQL Server, ClickHouse |
| **Confronto schemi**, **Confronto dati**, **Cerca un valore**, **Dati di prova**, **Sicurezza** | gli otto relazionali (PostgreSQL, CockroachDB, MySQL, MariaDB, SQL Server, SQLite, libSQL, DuckDB) e ClickHouse |
| **Backup e ripristino** | PostgreSQL, MySQL, MariaDB, SQL Server, SQLite, libSQL su file, DuckDB; ClickHouse solo il backup |

MongoDB non ha strumenti.

## Server
Solo per Redis e Valkey: versione, da quanto è acceso, comandi al secondo, client, memoria, chiavi. Sotto, **Informazioni** (le sezioni di `INFO`, con **Filtra**), **Client**, **Comandi lenti** e **Memoria**. Vedi [Redis e Valkey](redis).

## Attività
Le sessioni del server con **Utente**, **Applicazione**, **Stato**, da quanto, e la **Query** in corso. La riga in alto riassume: «Sessioni 12 · attive 3 · in attesa di lock 1».
- **Aggiorna ogni 3 s** rilegge da solo; **In pausa** lo ferma. **Nascondi le inattive** toglie le sessioni ferme.
- **Ferma la query** interrompe la query e lascia aperta la sessione; **Chiudi la sessione** la chiude e ne annulla la transazione. In produzione servono lo sblocco e la conferma.
- **Chi blocca chi** mostra le attese sui lock ad albero, come nel Monitoraggio.

## Monitoraggio
Una pagina sola per lo stato del server, aggiornata ogni 5 secondi (la tendina **Aggiornamento automatico** sceglie da 2 s a un minuto, o **In pausa**); la scelta resta anche chiudendo l'app. Gli aggiornamenti si fermano quando la pagina non si vede.

| Riquadro | Cosa dice |
|---|---|
| **Connessioni** | sessioni su quelle ammesse, attive, ferme in transazione, in attesa di lock |
| **Transazione più vecchia** | da quanto è aperta e chi la tiene; gialla oltre il minuto, rossa oltre i cinque |
| **Spazio del database** | quanto occupa il database aperto |
| **Letture dalla cache** | la quota di letture servite dalla memoria; gialla sotto il 90 % |

- **Chi blocca chi**: le sessioni in attesa stanno sotto quella che le blocca; in cima a ogni ramo c'è la sessione da chiudere, con **Chiudi la sessione**. Sotto ogni sessione in attesa c'è il lock che aspetta, quando il motore lo dice: su MySQL e MariaDB anche i lock dei metadati, come un `ALTER TABLE` fermo dietro una transazione lasciata aperta («lock dei metadati EXCLUSIVE su gestionale.ordini»). Il tasto destro su una riga ha anche **Spiega**.
- **Transazioni aperte** e **Database sul server** con le loro dimensioni.
- **Query lente**: le query che hanno preso più tempo, con esecuzioni, tempo totale e medio. **Spiega** apre la query in una scheda e ne disegna il piano stimato, senza eseguirla (vedi [Schema ER e piani di esecuzione](er-plans)). Su SQL Server la dichiarazione dei parametri di Query Store (`(@P1 int)SELECT …`) sparisce e restano i `@P1`; su CockroachDB ogni `_` diventa `$1`, `$2`…: la barra dei parametri chiede i valori, poi si preme Spiega.

Dove un dato manca, al suo posto c'è il motivo: «Questo utente non può leggerlo: serve PROCESS», oppure come attivare `pg_stat_statements`, `performance_schema` o Query Store. Per SQLite, libSQL e DuckDB la pagina mostra solo lo spazio. Su ClickHouse al posto delle transazioni ci sono le fusioni in corso.

## Spazio occupato
Per ogni tabella **Righe**, **Dati**, **Indici**, **Totale** e **Quota**. Sotto, **Indici** con dimensione e numero di letture: **Solo i mai usati** tiene quelli mai letti dall'ultimo azzeramento delle statistiche, **Copia i DROP** e **Apri nell'editor** preparano le istruzioni per toglierli. Un indice mai usato può servire a un report mensile: va verificato prima. Su ClickHouse compaiono anche le **Partizioni**.

## Utenti e permessi
A sinistra i ruoli e gli utenti, con **Filtra**; a destra quello scelto: attributi (**Può accedere**, **Superutente**, **Crea database**, **Crea ruoli**), **Membro di** e **Permessi sugli oggetti**.
1. Scegliere **Nuovo utente…**, **Nuovo ruolo…**, **Modifica…**, **Elimina…**, **Concedi…** o **Revoca…**.
2. Compilare la finestra: NiceDB scrive lo SQL in **SQL da eseguire**, modificabile.
3. **Esegui** lo manda dagli stessi controlli dell'editor; **Apri nell'editor** lo porta in una scheda.

Le password non passano mai dallo SQL generato: si impostano dall'editor con `ALTER ROLE` o `ALTER USER`.

## Confronto schemi
Confronta due schemi e scrive lo script che porta la destinazione allo stato dell'origine.
1. Scegliere connessione e schema per **Origine** (come deve essere, per esempio staging) e **Destinazione** (da allineare).
2. **Confronta**.
3. L'elenco mostra gli oggetti **Aggiunti**, **Rimossi**, **Modificati**; **Mostra anche gli uguali** aggiunge gli altri. Le note «perde dati» e «da ricostruire» segnano i passi delicati.
4. **Script di migrazione**: **Copia**, **Salva .sql** o **Apri nell'editor**. Lo script non parte mai da solo; con una destinazione di produzione si esegue solo dall'editor, con lo sblocco.

## Confronto dati
Confronta le righe di due query o tabelle, per chiave, anche su connessioni e motori diversi. Si apre anche dal tasto destro su un risultato › **Confronta con…**.
1. Per **Origine** e **Destinazione** scegliere **Query** o **Tabella**, la connessione e il testo o la tabella.
2. Scegliere la **Chiave**, le colonne che identificano una riga su entrambi i lati. NiceDB propone la chiave primaria.
3. **Confronta**. Il risultato conta le righe **Aggiunte**, **Tolte**, **Cambiate** e **Uguali**, con il valore di prima barrato accanto a quello nuovo.
4. **Script delle differenze**: prima i `DELETE`, poi gli `UPDATE` sulle sole colonne cambiate, infine gli `INSERT`, nel dialetto della destinazione. Si copia, si salva o si apre nell'editor; non parte da solo.

I valori si confrontano secondo il tipo (`12.50` e `12.5` sono uguali per un numero). Le righe arrivano in streaming e in sola lettura; per le query conviene un `ORDER BY` sulla chiave. Il confronto si ferma oltre 512 MB di righe in attesa.

## Cerca un valore
Cerca un testo o un numero in tutte le tabelle del database e dice dove l'ha trovato. Si apre anche dal tasto destro su un database o uno schema › **Cerca un valore…**
1. Scrivere il **Valore da cercare**.
2. Scegliere **Contiene** o **Uguale**, ed eventualmente **Maiuscole esatte**.
3. Restringere **Colonne** (**Testo**, **Numeri**, **Date, id e altri**), **Schemi** e **Tabelle** (parti del nome: `ord fatt`). **Anche le viste** aggiunge le viste, più lente da leggere.
4. Controllare la stima e premere **Cerca** o [[Invio]].

Ogni tabella con risultati mostra le colonne trovate con il loro conteggio e le prime righe. Clic su una colonna: la tabella si apre nella vista **Dati**, filtrata. La freccia su una riga (**Apri la riga**) la apre in una nuova scheda filtrata sulla chiave. **Righe per tabella** e **Tempo massimo** limitano il lavoro; **Annulla** ferma la ricerca. In produzione una ricerca pesante chiede conferma. Le query sono solo letture, ma **Contiene** non usa gli indici: su tabelle grandi legge tutto. I risultati restano se passi a un'altra vista e torni; una ricerca in corso però si ferma.

## Dati di prova
Riempie una tabella di righe inventate ma credibili, nei vincoli della tabella. Si apre anche dal tasto destro su una tabella › **Dati di prova…**
1. Scegliere **Tabella**, numero di **Righe** (da 1 a 1.000.000) e **Lingua**: **Italiano**, **Inglese (Regno Unito)**, **Inglese (Stati Uniti)**.
2. Per ogni colonna controllare **Valori** (il generatore: nome, email, codice fiscale, importo, data, **Uno fra…**, **Dalla tabella madre**…), i **Parametri** e **NULL %**.
3. Guardare l'**Anteprima**: le prime 20 righe come verranno scritte.
4. **Inserisci N righe** e confermare.

- NiceDB legge tipi, lunghezze, NOT NULL, chiavi, indici unici, CHECK, enum e chiavi esterne; le colonne automatiche le scrive il server. I CHECK tra più colonne si provano su ogni riga generata (elenco **Controlli della tabella**), e i valori unici già presenti nella tabella non si ripetono.
- Nome, cognome, email, città e codice fiscale di una riga vengono dalla stessa persona inventata; le email stanno solo su domini di esempio.
- In **Uno fra…** un peso con `*` rende un valore più frequente: `aperta*3, pagata*6, stornata`.
- Il **Seed** rende le righe ripetibili: stesso seed, stesse righe. **Nuovo seed** ne propone un altro.
- **Prima le tabelle madri** riempie prima le tabelle a cui questa punta.
- Tutto avviene in una transazione: al primo errore, o con **Annulla**, non resta nessuna riga.
- Su una connessione di produzione lo strumento non scrive, e non si può sbloccare.

## Backup e ripristino
Usa i programmi del motore, con l'avanzamento reale.

| Motore | Backup | Ripristino |
|---|---|---|
| PostgreSQL | **Archivio pg_dump** o **Script SQL**, anche **Solo la struttura, senza dati** | `pg_restore` o `psql`, in una sola transazione |
| MySQL, MariaDB | **Script SQL** con `mysqldump` | il client `mysql`; un errore a metà lascia il lavoro fatto fin lì |
| SQLite, libSQL su file | **Copia del file**, coerente anche mentre è in uso | sostituisce il contenuto del database |
| DuckDB | **Cartella** con `EXPORT DATABASE` | `IMPORT DATABASE`, che aggiunge le tabelle |
| SQL Server | **File .bak** scritto dal server sul suo disco | dall'editor, con `RESTORE DATABASE` dal database master |

1. Scegliere il formato, poi **Salva il backup…** (su SQL Server, il **Percorso sul server** e **Salva il backup sul server**).
2. Per ripristinare: **Scegli il file e ripristina…** e confermare. In produzione servono lo sblocco e il nome digitato.

Se `pg_dump` o `mysqldump` non si trovano, **Scegli la cartella dei programmi…** indica dove sono. CockroachDB e libSQL remoto non hanno backup da qui.

## Importa ed esporta
Esporta tabelle e risultati in CSV, TSV, JSON, Excel o Parquet e importa file in tabelle nuove o esistenti, in una sola transazione. Vedi [Importare ed esportare](import-export).

## Sicurezza
Il **Controllo di sicurezza** legge le impostazioni del server e dice cosa non va, con la correzione da copiare. È in sola lettura: non cambia niente e non prova password.
1. **Esegui il controllo**.
2. Leggere i problemi, raggruppati per gravità (**Alta**, **Media**, **Bassa**) e per categoria: **Account e permessi**, **Autenticazione e rete**, **Versione del server**, **Dati sensibili**, **Igiene**.
3. Ogni voce ha **Dove** e la **Correzione proposta**: **Copia SQL** o **Apri nell'editor**. Da rivedere prima di eseguire; in produzione passa dallo sblocco.
4. **Esporta PDF**, **Salva Markdown** o **Copia Markdown** per consegnare il rapporto.

In fondo, **Controlli superati** e **Non verificabili qui** (per motore, permessi o dati che il server non registra). Sui motori a file restano pochi controlli: dati sensibili e, per SQLite, la versione.

## Limiti
- Le letture del Monitoraggio sono cumulative dall'ultimo azzeramento delle statistiche del motore; la pagina non le azzera.
- Il confronto dati non reinserisce in modo affidabile i valori binari.
- I dati di prova prendono i valori delle chiavi esterne dalle prime 5.000 righe della tabella madre.
- L'elenco delle versioni vulnerabili del Controllo di sicurezza è incluso nell'app; **Aggiorna l'elenco**, in fondo al rapporto, scarica quello firmato dell'ultimo rilascio. Non sostituisce gli avvisi del produttore.
