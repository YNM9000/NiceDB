# Importare ed esportare

## Esportare una tabella
1. Tasto destro sulla tabella nell'esploratore › **Esporta dati…**. In alternativa: **Strumenti › Importa ed esporta**, scegli la tabella e premi **Esporta…**; oppure il comando «Esporta *tabella* in…» della palette, che chiede subito il formato.
2. Scegli il **Formato**:

   | Formato | Per cosa |
   |---|---|
   | CSV | virgole, per ogni programma |
   | TSV | tabulazioni, da incollare |
   | JSON | numeri e JSON con il loro tipo |
   | XLSX | un foglio, numeri come numeri |
   | Parquet | colonnare e compresso |

3. Per CSV, TSV e XLSX lascia o togli **Prima riga con i nomi delle colonne**.
4. **Esporta…** e scegli dove salvare il file.

- Il file si scrive mentre la tabella si legge: non c'è limite di righe in memoria.
- La barra mostra l'avanzamento; **Annulla** ferma tutto ed elimina il file parziale.
- Alla fine: righe scritte, nome del file, dimensione e tempo.
- Un foglio Excel tiene al massimo 1.048.576 righe: oltre, usa CSV o Parquet.

## Esportare il risultato di una query
1. Esegui la query.
2. **Esporta** sopra i risultati, oppure tasto destro sulla scheda dei risultati › **Esporta**.
3. Scegli formato e file come per una tabella.

La query viene rieseguita su una connessione a parte e scritta mentre arriva, quindi il file contiene tutte le righe, non solo quelle nella griglia. Si esporta solo il risultato di una `SELECT`.

Per poche righe basta la griglia: tasto destro › **Copia come** › **Testo con intestazioni**, **Come CSV**, **Come JSON** o **Come INSERT**. Vedi [Eseguire e leggere i risultati](results).

## Importare un file in una tabella
1. Apri l'importazione:
   - tasto destro su una tabella › **Importa dati…**, per importare in quella tabella;
   - **Strumenti › Importa ed esporta › Importa un file…**;
   - il comando «Importa dati in *tabella*…» della palette.
2. **Scegli un file…**: CSV e TSV (separatore rilevato, anche punto e virgola), JSON (un array di oggetti o un oggetto per riga), XLSX, Parquet. Il file non viene caricato tutto in memoria.
3. Controlla l'anteprima. Se serve, cambia **Prima riga con i nomi** e, per un file Excel, il **Foglio**.
4. Scegli la **Destinazione**:
   - **Tabella esistente**: in **Corrispondenza delle colonne** ogni colonna della tabella prende una colonna del file, abbinata per nome quando si può; «— lascia vuota» la salta.
   - **Nuova tabella**: scrivi il **Nome della tabella**; per ogni colonna del file scegli se importarla, il **Nome della colonna** e il **Tipo** proposto.
5. Lascia o togli **Celle vuote come NULL**.
6. Scegli cosa fare delle **Righe con errori**:
   - **Fermati al primo errore**: una riga sbagliata annulla tutto, la tabella resta com'era;
   - **Salta e prosegui**: le righe sbagliate restano fuori e vengono elencate, il resto viene confermato.
7. Premi il pulsante in basso («Importa 5 colonne») e conferma «Importare in *tabella*?» con **Importa**.

- L'importazione avviene in una sola transazione. **Annulla** durante il lavoro non scrive nessuna riga.
- Alla fine: «Importate 12.400 righe in clienti in 3 s», più le **Righe scartate** con numero di riga e motivo (le prime 500).
- Su una connessione di produzione la finestra mostra l'interruttore della scrittura e l'importazione chiede la conferma scelta in Impostazioni: vedi [Sicurezza in produzione](production).

## Passare connessioni a un collega
Un **Pacchetto cifrato** (`.nicedb`) porta cartelle, connessioni e, a scelta, altro, protetto da una frase segreta.

1. Nella barra laterale, **Altre azioni** (i tre puntini) › **Esporta…** per tutto lo spazio di lavoro attuale; oppure tasto destro su una cartella o una connessione › **Esporta…**.
2. In **Formato** scegli **Pacchetto cifrato** o **Definizioni JSON**.
3. In **Cosa esportare** spunta cartelle e connessioni.
4. Solo per il pacchetto cifrato, in **Insieme**: gli spazi di lavoro, **La cronologia delle query**, **Gli script, gli snippet e le note**, **Le impostazioni di aspetto**, **Le password dei database**.
5. Scrivi la **Frase segreta** (almeno 8 caratteri) e **Ripeti la frase**.
6. **Esporta…** e scegli il file.

- Manda la frase segreta per un'altra strada. Senza la frase il pacchetto non si apre e non c'è modo di recuperarla.
- **Le password dei database** restano solo dentro il pacchetto cifrato: chi conosce la frase può leggerle. Le passphrase di SSH, proxy e certificati non partono mai: chi importa usa le sue.
- **Definizioni JSON** porta solo cartelle e connessioni, in chiaro, senza password né segreti SSH.

## Importare un pacchetto
1. **Altre azioni** › **Importa…** e scegli un file `.nicedb` o `.json`.
2. Se il pacchetto è cifrato, scrivi la **Frase segreta** e premi **Apri**.
3. Controlla l'elenco. Le voci nuove sono segnate «nuova»; per quelle che esistono già scegli cosa fare:

   | Voce | Scelte |
   |---|---|
   | Connessione, spazio di lavoro | **Tieni entrambe** (di serie), **Sostituisci**, **Salta** |
   | Cartella | **Unisci** (di serie), **Separata** |
   | Impostazioni di aspetto | **Tieni le mie** (di serie), **Sostituisci** |

4. **Importa**. Il riepilogo dice quante voci sono state aggiunte, sostituite e lasciate com'erano.

Nulla viene sovrascritto senza una scelta.

## Altri dati da esportare
- Il registro delle scritture si esporta in CSV o JSON dal pannello **Registro** della vista Dati: vedi [Sicurezza in produzione](production).
- Il backup di un database con gli strumenti del motore è in **Strumenti › Backup e ripristino**: vedi [Strumenti](tools).

## Limiti
- Fissati e segnalibri della palette e i profili dei dati di prova non viaggiano nei pacchetti.
- Un pacchetto fatto con una versione più recente di NiceDB non si apre con una più vecchia.
- Un file Excel si importa un foglio alla volta.
