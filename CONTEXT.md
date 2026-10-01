# Mango Domain Glossary

Shared vocabulary for describing Mango's library and reading state. Use Mango's domain terms consistently across the Crystal reference and Rust port.

## Library

**Library**:
The collection of manga titles available to a reader, including titles nested under other titles.

**Title**:
A named manga collection that may contain readable entries, child titles, or both.
_Avoid_: Book, when referring to the domain entity.

**Parent title**:
The containing title directly above a nested title.

## Reading content

**Entry**:
A readable unit within a title, consisting of an archive or a directory of loose image pages.
_Avoid_: Book, chapter, volume, when referring generically to the content unit.

**Page**:
One ordered image in an entry.

## Reading state

**Progress**:
A user's position in an entry, measured as the number of pages read. Zero means unread.

**Date added**:
The time an entry was first added to the library; it remains associated with that entry when the library is rescanned.
