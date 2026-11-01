# Vista Dati e modifiche

La vista **Dati** mostra una tabella a pagine da 500 righe e la lascia modificare come un foglio di calcolo. Le modifiche restano in sospeso finché non si preme **Applica**, che le scrive tutte insieme in una transazione.

## Aprire una tabella
- Clic su una tabella nell'esploratore, oppure tasto destro › **Apri i dati**.
- Nel selettore delle viste, **Dati**, poi la tendina **Scegli una tabella** in testata: il campo **Filtra tabelle** restringe l'elenco.
- Dalla palette: [[key:palette]], scrivere il nome della tabella, [[Invio]].

In testata si leggono il numero di righe (con la tilde quando è una stima, per esempio «~3.000.000 righe»), lo schema e la chiave usata per riconoscere le righe. Le frecce in fondo a destra cambiano pagina; **Ricarica** rilegge la pagina dal server.

## Filtrare e ordinare
- **Condizione WHERE**: si scrive la condizione dopo `WHERE`, per esempio `fido > 1000 and provincia = 'BG'`, e si conferma con [[Invio]].
- **Filtro** apre un piccolo modulo: **Colonna**, **Condizione** (`=`, `≠`, `contiene`, `<`, `≤`, `>`, `≥`, `è NULL`, `non è NULL`), **Valore**, poi **Aggiungi**. Ogni filtro diventa un chip accanto al WHERE; la × lo toglie.
- Tasto destro su una cella › **Filtra per questo valore** aggiunge il chip con quel valore.
- Clic sull'intestazione di una colonna ordina sul server; con [[Shift]] premuto aggiunge la colonna all'ordinamento. Il menu dell'intestazione ha **Ordina crescente**, **Ordina decrescente** e **Togli ordinamento**.

Filtri e ordinamento si annullano con [[Ctrl+Z]] come ogni altra azione.

## Modificare le righe
1. Selezionare una cella e cominciare a scrivere, oppure premere [[F2]], [[Invio]] o fare doppio clic.
2. Confermare con [[Invio]] (scende di una riga) o [[Tab]] (passa alla colonna dopo); [[Esc]] lascia il valore com'era.
3. Ripetere sulle altre celle. Ogni cella cambiata resta evidenziata e la barra in basso conta le modifiche: «3 celle modificate · 1 riga nuova».
4. Premere **Applica** (o [[Ctrl+S]]).

![Modificare una cella, vedere le modifiche in sospeso e applicarle](anim:data-edit)

| Tasto | Effetto |
|---|---|
| [[F2]], [[Invio]], doppio clic | modifica la cella |
| [[Backspace]] | modifica la cella partendo da vuota |
| [[Canc]] | imposta la cella a NULL |
| [[Ctrl+Canc]] | segna la riga per l'eliminazione, o la ripristina |
| [[Ctrl+Z]] | annulla l'ultima modifica in sospeso |
| [[Ctrl+Shift+Z]], [[Ctrl+Y]] | ripete la modifica annullata |
| [[Ctrl+S]] | applica |

- **Nuova riga** aggiunge una riga vuota in cima, segnata «nuova». Le colonne lasciate vuote prendono il valore predefinito del server.
- Tasto destro su una cella o sul numero di riga: **Modifica cella**, **Imposta NULL**, **Elimina riga** (o **Elimina righe** con più righe selezionate), **Ripristina riga**, **Togli riga nuova**.
- Le colonne binarie si scrivono in esadecimale, `\x0a1f` o `0x0A1F`.
- Le colonne con chiave esterna aprono un elenco delle righe collegate mentre si scrive: vedi [Navigazione delle chiavi esterne](fk).
- I valori lunghi, il JSON e l'XML si modificano più comodamente dal [Pannello valore](value).

## Controllare prima di applicare
La barra in basso ha **Mostra SQL**: apre l'elenco delle istruzioni che **Applica** eseguirà, una per modifica, con la nota «SQL che verrà eseguito · transazione unica». Accanto a ogni istruzione la freccia **Annulla questa modifica** toglie solo quella; **Copia SQL** copia tutto. **Annulla tutto** scarta ogni modifica in sospeso.

## Applicare
- Tutte le modifiche partono in una sola transazione: se una fallisce, non viene scritto niente e la riga colpevole resta segnata con il messaggio del server.
- Prima di aggiornare o cancellare, NiceDB controlla che la riga sul server sia ancora quella letta. Se qualcuno l'ha cambiata nel frattempo, compare «Cambiata da qualcun altro dopo la lettura» e non si applica nulla: si ricarica e si riprova.
- Se la scheda non conferma da sola (commit manuale o intelligente, o una transazione aperta) le modifiche finiscono nella transazione e vanno confermate con **Commit**: vedi [Transazioni e prova a secco](transactions).
- Su una connessione di produzione la vista resta in sola lettura finché non si sblocca la scrittura, e **Applica** chiede conferma: vedi [Sicurezza in produzione](production).
- Dopo un **Applica** riuscito non c'è più niente da annullare nella vista: le modifiche sono sul server.

## Quando non si può modificare
Al posto di **Nuova riga** compare un lucchetto con il motivo:
- «Sola lettura: è una vista.»
- «Sola lettura: la tabella non ha una chiave primaria né univoca.» Senza una chiave NiceDB non sa quale riga cambiare.
- «Produzione in sola lettura: sblocca la scrittura per modificare.»
- Su ClickHouse le righe non si modificano una per una: si usano `INSERT`, `ALTER TABLE … UPDATE` o `ALTER TABLE … DELETE` nell'editor.

## Registro delle scritture
**Registro**, in testata, apre a destra il registro delle scritture della connessione: ogni modifica applicata, con ora, utente, database e righe toccate, insieme alle scritture fatte dall'editor e dagli strumenti. **Esporta** lo salva come **Tabella CSV** o **File JSON**. Il registro si alterna con il pannello **Collegamenti** e con il pannello valore: se ne vede uno alla volta.

## Copiare
- [[Ctrl+C]] copia le celle selezionate come testo con tabulazioni, pronto da incollare in un foglio di calcolo.
- Tasto destro › **Copia come**: **Testo con intestazioni**, **Come CSV**, **Come JSON**, **Come INSERT**.
- Sull'intestazione: **Copia nome**, **Copia colonna**, **Adatta larghezza** (anche con il doppio clic sul bordo della colonna).

## Limiti
- Si modifica la pagina caricata: per cambiare migliaia di righe conviene un `UPDATE` dall'editor.
- La vista non modifica le viste, anche quando il motore le renderebbe aggiornabili.
- Grafico, pannello valore e aggregati lavorano sulla pagina corrente, non su tutta la tabella.
