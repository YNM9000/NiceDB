# Firebird

NiceDB parla il protocollo di Firebird da solo: non serve installare fbclient.

## Connettersi
1. **Nuova connessione**, poi in **Motore** scegli Firebird.
2. **Host** e **porta** (3050), poi **Database**: il percorso del file sul server (`/var/lib/firebird/data/gestionale.fdb`) o il suo alias.
3. **Utente** e **password**. La password sta nel portachiavi. Firebird cifra il protocollo da sé: non c'è un'opzione TLS.
4. Tunnel SSH e proxy funzionano come per gli altri motori.

## Esplorare
Firebird non ha schemi prima della versione 6: tutto sta in un solo gruppo. Ci sono **tabelle**, **viste**, **procedure**, **generatori** e **trigger**. Dal tasto destro: **Mostra DDL** anche per gli indici (un indice di una chiave diventa il suo `ALTER TABLE … ADD CONSTRAINT`) e **Dipendenze**, lette da `RDB$DEPENDENCIES` e dalle chiavi esterne.

## Query
- I risultati vuoti hanno comunque le intestazioni, con il tipo di ogni colonna.
- NUMERIC e DECIMAL arrivano esatti (`12.50`), non arrotondati.
- **Ferma** annulla davvero la query: NiceDB apre una seconda connessione e cancella l'istruzione da `MON$STATEMENTS`. La connessione resta aperta.
- `SET TRANSACTION`, `COMMIT` e `ROLLBACK` aprono e chiudono una transazione; il resto va in commit automatico.

## Spiega
Il piano è quello che Firebird sceglie quando prepara l'istruzione: tabelle, indici, join, ordinamenti. Firebird non misura le esecuzioni, quindi il piano è sempre una stima.

## Strumenti
- **Attività**: le connessioni da `MON$ATTACHMENTS` con l'istruzione in corso; **Ferma** annulla l'istruzione, **Chiudi** chiude la connessione.
- **Monitoraggio**: connessioni, transazioni aperte, dimensione del database e uso della cache.
- **Spazio**: le tabelle con le righe stimate dalle statistiche della chiave primaria. Firebird non dice quanto spazio occupa ogni tabella.
- **Utenti**: gli utenti di `SEC$USERS` (tutti per un amministratore, solo il proprio altrimenti), i ruoli e i permessi.
