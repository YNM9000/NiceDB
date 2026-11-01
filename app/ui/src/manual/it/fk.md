# Navigazione delle chiavi esterne

Nella vista **Dati** una chiave esterna porta alla riga a cui punta, una riga mostra chi la richiama, e un percorso in testata riporta indietro. Valgono le chiavi esterne dichiarate nel catalogo del server; le relazioni senza vincolo non compaiono.

## Riconoscere una chiave esterna
- L'intestazione della colonna ha il simbolo della catena; il suggerimento dice la destinazione, per esempio «Chiave esterna verso clienti (id)».
- I valori sono sottolineati a puntini. Sulla cella sotto il mouse o selezionata compare una piccola freccia.
- La barra in basso aggiunge «F4 segue la chiave».

## Aprire la riga collegata
1. Selezionare una cella della chiave esterna.
2. Premere [[F4]], o fare clic sulla freccia della cella.
3. La tabella di destinazione si apre nella stessa scheda, filtrata sulla riga: il filtro è un chip (`id = 63`) accanto al WHERE.

![Seguire una chiave esterna fino alla riga collegata e tornare indietro](anim:fk-navigation)

Per aprirla in una **nuova scheda**, lasciando com'è quella di partenza: [[Shift+F4]], clic centrale o [[Ctrl]]+clic sulla freccia, oppure tasto destro › **Apri in una nuova scheda**. La scheda nuova prende il nome della riga, per esempio `clienti (id = 63)`. Il menu ha anche **Apri la riga collegata**.

Se la chiave vale NULL non c'è niente da aprire e la barra lo dice: «Niente da aprire: cliente_id è NULL.» Con modifiche in sospeso la tabella non cambia: prima si applicano o si annullano.

## Tornare indietro
Ogni passo lascia una briciola a sinistra del nome della tabella: `ordini › clienti › fatture`.
- Clic su una briciola: si torna a quel punto, con i filtri, l'ordinamento e la pagina di allora.
- [[Alt+←]], la freccia accanto alle briciole o il tasto «indietro» del mouse: un passo indietro.
- [[Alt+→]] o il tasto «avanti» del mouse: rifà il passo annullato.

Dopo due passi le briciole più vecchie si raccolgono in «…»; il percorso completo è nel suggerimento. Scegliere una tabella dalla tendina ricomincia da capo.

Il percorso resta con la scheda: torna se la riapri con [[Ctrl+Z]] dopo averla chiusa, e lo spazio di lavoro lo salva con filtri, ordinamento e pagina per la prossima volta che apri NiceDB.

## Collegamenti della riga
**Collegamenti**, in testata, apre un pannello a destra che segue la riga selezionata. Compare solo se la tabella ha chiavi esterne, in uscita o in entrata. [[F4]] su una colonna che non è una chiave esterna apre lo stesso pannello.
- **Punta a**: una voce per ogni chiave esterna della tabella, con la colonna di partenza («da cliente_id») e un'anteprima della riga di destinazione. Clic per aprirla qui, l'icona a destra per aprirla in una nuova scheda.
- **Puntano qui**: le tabelle che hanno una chiave esterna verso questa, anche da altri schemi, con il numero di righe che richiamano la riga selezionata. Clic per aprire quelle righe filtrate.

I conteggi girano in sola lettura e si fermano dopo 2 secondi; quelli non arrivati in tempo restano «—». Il pannello si allarga dal bordo sinistro e si alterna con il Registro e il pannello valore.

## Scegliere il valore di una chiave esterna
Modificando una cella di chiave esterna, sotto la cella si apre l'elenco delle righe della tabella di destinazione, con la chiave e due colonne descrittive.
1. Senza scrivere nulla l'elenco mostra le prime righe, con quella attuale in cima.
2. Scrivendo, l'elenco cerca il testo in tutte le colonne mostrate, senza distinguere maiuscole e minuscole, e mostra le prime 40. Ogni tasto ferma la ricerca precedente; dopo 3 secondi senza risultati la ricerca si ferma e chiede un testo più preciso.
3. Scegliere con [[↑]] [[↓]] o il clic, poi confermare.

| Tasto | Effetto |
|---|---|
| [[↑]] [[↓]], [[Pag↑]] [[Pag↓]] | scorre l'elenco |
| [[Invio]] | conferma e scende alla riga sotto |
| [[Tab]] | conferma e resta sulla riga |
| [[Esc]] | annulla |

Se non si sceglie niente, [[Invio]] salva il testo scritto, come in ogni altra cella. Il valore scelto è una modifica in sospeso: si annulla con [[Ctrl+Z]] e va al server con **Applica** (vedi [Vista Dati e modifiche](data)).

## Chiavi su più colonne
Una chiave esterna su più colonne funziona ovunque: [[F4]] apre la riga con un chip per colonna, i conteggi confrontano tutte le colonne, l'elenco di scelta le riempie tutte insieme. Se una colonna fa parte di più chiavi, [[F4]] segue quella a una sola colonna.

## Motori
PostgreSQL, CockroachDB, MySQL, MariaDB, SQL Server, SQLite, libSQL e DuckDB.

## Limiti
- Le viste non hanno chiavi esterne nel catalogo: niente frecce né collegamenti.
- Su una tabella grande senza indice sulla colonna della chiave il conteggio di «Puntano qui» può non arrivare in tempo.
- La ricerca dell'elenco di scelta confronta il testo: su tabelle enormi senza indici adatti è una scansione, fermata alle prime 40 righe o dopo 3 secondi. Il totale compare solo quando le righe trovate stanno tutte nell'elenco.
