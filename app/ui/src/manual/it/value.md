# Pannello valore e grafici

Il **pannello valore** apre a destra della griglia il contenuto intero di una cella: JSON ad albero, XML indentato, testi lunghi, immagini, binari. Funziona nei risultati delle query e nella vista **Dati**. Nella stessa area della griglia ci sono gli aggregati della selezione e il grafico dei risultati.

## Aprire il pannello
1. Selezionare una cella nella griglia.
2. Premere [[Shift+Invio]], oppure **Valore** nella barra dei risultati o nella testata della vista Dati.
3. Muoversi nella griglia con il mouse o le frecce: il pannello segue la cella selezionata.

![Aprire il pannello valore su una cella JSON](anim:value-panel)

- [[Alt+Invio]] apre la stessa cella nella scheda **Riga**, cioè la riga intera in colonna.
- Dalla palette dei comandi ([[key:palette_commands]]): «Mostra il valore della cella» e «Mostra la riga come scheda».
- [[Esc]] dentro il pannello lo chiude; ripetere [[Shift+Invio]] fa lo stesso.
- Il pannello si allarga dal bordo sinistro e la larghezza resta salvata nello spazio di lavoro.

In alto si leggono la colonna, il tipo, la riga e cosa contiene (`JSON`, `XML`, `PNG`, `PDF`, **Testo**, **Binario** o `NULL`), con la lunghezza o la dimensione.

## Leggere il valore
NiceDB riconosce il contenuto dal tipo della colonna e poi dal valore: un testo che comincia con `{` o `[` ed è JSON valido si apre come JSON anche in una colonna `text`.

### JSON
- **Albero** mostra il documento a nodi; un oggetto chiuso mostra quante chiavi ha e un'anteprima. Gli array lunghi mostrano cento elementi alla volta, con «Altri 400…» in fondo.
- **Testo** mostra lo stesso documento indentato, con i numeri di riga.
- **Cerca** filtra per chiave o per valore e apre i rami dove trova qualcosa. [[Invio]] va al risultato dopo, [[Shift+Invio]] a quello prima.
- In fondo c'è il percorso del nodo scelto, per esempio `$.cliente.indirizzi[0].cap`. L'icona accanto lo copia; **SQL** copia l'espressione che legge quel valore nel dialetto della connessione:

| Motore | Espressione copiata |
|---|---|
| PostgreSQL, CockroachDB | `dati #>> '{cliente,indirizzi,0,cap}'` |
| MySQL, MariaDB | `JSON_UNQUOTE(JSON_EXTRACT(dati, '$.cliente.indirizzi[0].cap'))` |
| SQL Server | `JSON_VALUE(dati, '$.cliente.indirizzi[0].cap')` |
| SQLite, libSQL, DuckDB | `json_extract(dati, '$.cliente.indirizzi[0].cap')` |

Tasto destro su un nodo: **Copia il percorso**, **Copia come espressione SQL**, **Copia il valore**, **Apri tutto qui sotto**, **Chiudi tutto qui sotto**.

### Altri contenuti
- **XML**: indentato un elemento per riga, con i colori dell'editor. Se i tag non si chiudono bene il testo resta com'era.
- **Testo**: va a capo nel pannello, con i numeri di riga. Oltre le 2000 righe compare «Mostra altre N righe».
- **Immagini** PNG, JPEG, GIF, WebP, BMP, ICO e TIFF (la prima pagina): anteprima su una scacchiera, con dimensione in byte e in pixel.
- **SVG**, sia in una colonna di testo sia in un binario: disegnato come immagine, mai inserito nella pagina, quindi script e collegamenti dentro il file non partono. Per un SVG di testo **Immagine** e **Testo** passano dal disegno al sorgente.
- **Altri binari**: dimensione, tipo quando si riconosce (PDF, ZIP, GZIP) e i primi 256 byte in esadecimale.

**Copia il valore** copia sempre il valore originale. **Salva su file** lo salva com'è, con un nome come `logo-12.png` o `dati-7.json`.

## Modificare un valore
Nella vista **Dati**, su una tabella modificabile:
1. Premere **Modifica**, o [[F2]] con il pannello in primo piano.
2. Cambiare il testo. In una colonna `json` o `jsonb`, **Formatta** lo indenta.
3. **Salva** ([[Ctrl+Invio]]) lo scrive come modifica in sospeso; **Annulla** o [[Esc]] lasciano tutto com'era; **NULL** imposta il valore a NULL.
4. Premere **Applica** nella vista Dati per mandarlo al server (vedi [Vista Dati e modifiche](data)).

Un JSON non valido non si salva: il pannello dice dove si rompe. Nei risultati delle query il pulsante non c'è; in una vista, su una tabella senza chiave, su una riga da eliminare o in produzione senza sblocco resta spento, con il motivo nel suggerimento.

### Binari
In una colonna binaria (`bytea`, `blob`, `varbinary`…) al posto di **Modifica** ci sono:
- **Carica da file…**: sceglie un file (fino a 16 MB) e ne mette i byte nella cella;
- **Esadecimale** ([[F2]]): i byte in coppie di cifre, otto per riga, da correggere a mano; spazi e a capo non contano. Oltre 1 MB si carica un file.

Tutti e due diventano una modifica in sospeso come le altre: si vede nella barra in basso, si annulla con [[Ctrl+Z]] e va al server con **Applica**.

## La scheda Riga
**Riga** mostra la riga della cella selezionata in colonna: nome e tipo a sinistra, valore a destra.

| Tasto | Effetto |
|---|---|
| [[↑]] [[↓]] | riga prima o dopo |
| [[Pag↑]] [[Pag↓]] | dieci righe indietro o avanti |
| [[Ctrl+Home]], [[Ctrl+Fine]] | prima e ultima riga |
| [[Tab]], [[Shift+Tab]] | campo dopo o prima |
| [[Invio]] | apre il campo nella scheda **Valore** |
| [[Ctrl+C]] | copia il valore del campo |

**Copia la riga** copia una riga per colonna, `nome<TAB>valore`.

## Aggregati della selezione
Con più celle selezionate, a destra nella barra di stato compaiono i conti, come in un foglio di calcolo:
- celle numeriche: **Conteggio**, **Somma**, **Media**, **Min**, **Max**;
- altre celle: **Conteggio** e **Distinti**;
- in tutti e due i casi, se ce ne sono, quanti NULL.

Il conteggio non conta i NULL. La somma dei decimali è esatta (`0.1 + 0.2` fa `0.3`). Clic su un aggregato per copiarlo. Con molte celle la barra mostra «Calcolo…» finché il conto non arriva.

## Grafico
La scheda **Grafico**, accanto a **Risultati** e **Messaggi**, disegna le righe già lette senza rieseguire la query. Nella vista Dati il pulsante con il grafico in testata mostra il grafico della pagina al posto della griglia. Dalla palette: «Mostra il grafico».

NiceDB propone un primo grafico: con una colonna data su X disegna linee, altrimenti barre sulla prima colonna di testo; su Y la prima colonna numerica che non sia una chiave; se X si ripete somma i valori e tiene le prime 12 categorie.

| Scelta | Valori |
|---|---|
| **Tipo di grafico** | **Barre**, **Barre orizzontali**, **Linee**, **Aree**, **Torta**, **Dispersione** |
| **X** | la colonna delle categorie o dell'asse orizzontale |
| **Y** | una o più colonne numeriche |
| **Aggregazione** | **Nessuna**, **Somma**, **Conteggio**, **Media**, **Minimo**, **Massimo**, raggruppando per X |
| **Ordine** | **Arrivo**, **Per X**, **Decrescente**, **Crescente** |
| **Categorie** | **Tutte** o **Prime 5, 10, 20, 50**; il resto finisce in **altri** |

- La torta usa solo la prima Y e solo i valori positivi. La dispersione vuole una X numerica o una data e non aggrega.
- Passando con il mouse si legge il dettaglio. Da tastiera: [[Tab]] fino al grafico, poi le frecce, [[Home]] e [[Fine]].
- **Esporta** salva **Immagine SVG** o **Immagine PNG**, con i colori dello stile in uso.
- Le scelte restano legate alla scheda: rilanciando la query il grafico è quello di prima.

## Limiti
- Pannello, aggregati e grafico usano le righe già arrivate alla griglia: nei risultati quelle lette fino al limite, nella vista Dati la pagina corrente. Quando ce ne sono altre, la barra di stato (con colonne intere selezionate) offre **Tutte le righe** e il grafico (con un'aggregazione) **Raggruppa tutte le righe**: il conto lo fa il server, con una sola query di sola lettura.
- L'anteprima delle immagini si ferma a 16 MB; di un TIFF si vede la prima pagina.
- Il grafico disegna al massimo 5000 categorie o punti.
- Niente assi doppi e niente aree impilate.
