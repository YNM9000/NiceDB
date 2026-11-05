# Spazi di lavoro e cartelle

## Cartelle
Le cartelle raggruppano le connessioni nella colonna **Connessioni**, per cliente o per progetto, e possono contenere altre cartelle.

1. Premi l'icona **Nuova cartella** accanto a **Connessioni**, oppure, su una cartella, tasto destro › **Nuova sottocartella**.
2. Scrivi il nome e premi [[Invio]]; [[Esc]] rinuncia.

| Azione | Come |
|---|---|
| Rinominare | [[F2]] sulla cartella, o tasto destro › **Rinomina** |
| Aprire e chiudere | clic, oppure [[→]] e [[←]] |
| Dare un colore | tasto destro › **Colore** |
| Dare un segno (stella, azienda, server…) | tasto destro › **Segno** |
| Spostare | trascinare la cartella o la connessione; tasto destro › **Sposta in**; [[Alt+↑]] e [[Alt+↓]] cambiano l'ordine |
| Eliminare | tasto destro › **Elimina cartella**: il contenuto resta, sale di un livello |

Durante il trascinamento una cartella chiusa si apre se ci si ferma sopra; [[Esc]] annulla lo spostamento. Il numero accanto alla cartella conta le connessioni che contiene.

Il campo **Cerca connessioni** filtra l'albero mentre scrivi; [[Invio]] passa la ricerca a **Cerca ovunque**.

## Spazi di lavoro
Uno spazio di lavoro mostra solo alcune cartelle e connessioni e ricorda le sue schede. **Tutte le connessioni** è lo spazio che mostra tutto.

### Creare uno spazio
1. Apri il menu dello spazio nella barra del titolo e scegli **Nuovo spazio di lavoro…**.
2. Scrivi il **Nome**.
3. In **Cartelle e connessioni** spunta cosa deve contenere. Una cartella porta con sé tutto quello che contiene, anche ciò che aggiungerai dopo.
4. Premi **Crea**: NiceDB passa subito al nuovo spazio.

Dall'albero: tasto destro su una cartella o una connessione › **Spazi di lavoro** › **Nuovo spazio con questa…**, oppure la spunta su uno spazio esistente per aggiungerla o toglierla.

### Passare da uno spazio all'altro
- Menu dello spazio nella barra del titolo, poi lo spazio.
- [[Ctrl+Alt+1]] per **Tutte le connessioni**, [[Ctrl+Alt+2]] … [[Ctrl+Alt+9]] per gli spazi nell'ordine del menu.
- Dalla palette: «Passa allo spazio di lavoro…».

Cambiando spazio, le schede dello spazio lasciato si chiudono e tornano quelle del nuovo. Una scheda ripristinata si collega solo quando la apri. Se una sessione ha una transazione aperta con scritture, NiceDB chiede prima cosa farne: vedi [Transazioni e prova a secco](transactions).

### Cosa ricorda uno spazio
- le schede aperte, con testo, titolo, vista, tabella e database;
- la scheda attiva;
- la divisione tra editor e risultati e se la cronologia era aperta;
- la larghezza delle colonne laterali.

Una connessione creata o spostata mentre uno spazio è attivo entra nello spazio, così resta visibile.

### Modificare, duplicare, eliminare
Dal menu dello spazio: **Modifica «nome»…**, **Duplica** ed **Elimina…**. Eliminare uno spazio non tocca le connessioni: si perdono solo le schede aperte in quello spazio. [[Ctrl+Z]] annulla la creazione, la modifica e l'eliminazione.

## Variabili dello spazio di lavoro
Le variabili danno un valore ai campi delle connessioni scritti come `${NOME}`, per esempio `db.${AMBIENTE}.bassi.lan`.
1. Apri **Modifica «nome»…** dal menu dello spazio, oppure il comando «Variabili dello spazio di lavoro…» nella palette.
2. In **Variabili** premi **Aggiungi variabile** e scrivi **Nome** e **Valore**.
3. Premi **Salva**.

- Valgono quando lo spazio è attivo e hanno la precedenza sull'ambiente del sistema.
- Un nome usa lettere, cifre e `_` e non comincia con una cifra; nomi doppi o che sembrano password bloccano il salvataggio.
- Con **Tutte le connessioni** valgono le variabili del primo spazio che contiene la connessione.

Come si scrivono nei campi: [Connessioni e profili](connections).

## Condividere cartelle e connessioni
Tasto destro su una cartella o una connessione › **Esporta…**, oppure **Altre azioni** (i tre puntini accanto a **Connessioni**) › **Esporta…** per tutto lo spazio.

| Formato | Contenuto |
|---|---|
| **Pacchetto cifrato** | un file `.nicedb` protetto da una **Frase segreta** di almeno 8 caratteri; a scelta spazi di lavoro, cronologia, impostazioni di aspetto, script, snippet e note, password dei database |
| **Definizioni JSON** | solo cartelle e connessioni, in chiaro, senza password né segreti SSH |

Per caricarle: **Altre azioni** › **Importa…**. Per ogni voce che esiste già si sceglie cosa fare, per esempio **Tieni entrambe**, **Sostituisci** o **Salta**: nulla viene sovrascritto senza una scelta. La frase segreta non si recupera; mandala per un'altra strada. Altri formati in [Importare ed esportare](import-export).

## Schede
| Azione | Come |
|---|---|
| Nuova scheda sulla stessa connessione | **+** dopo le schede, o [[key:new_tab]] |
| Rinominare | doppio clic sul titolo o [[F2]]; [[Invio]] conferma, [[Esc]] rinuncia |
| Duplicare, chiudere le altre, chiudere quelle a destra | tasto destro sulla scheda |
| Scorrere le schede | rotella del mouse sulla riga delle schede |
| Riaprire una scheda chiusa | [[Ctrl+Z]] |

Il suggerimento sulla scheda dice database, query in corso, modifiche non applicate, transazione aperta e produzione. Quando una connessione ha più database aperti, il nome del database compare accanto al titolo.
