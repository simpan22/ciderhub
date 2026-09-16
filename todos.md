Todo features and bugs in priority order:
When starting on each of these entries, create a docs/specs/<feature_name>.md file with a more detailed spec first for review before implementation.

# Feature: Units (Status: Planning)
When adding additives we should select units from a dropdown list. One should be able to add to that list so we need to store it in a table. Handle existing units by adding a lowercase version of the unit to the new units table. When adding a new unit, we should check if it already exists in the table (case insensitive). If it does, we should use the existing unit instead of creating a new one.

# Feature: Easier input forms (Status: Planning)
When for example the juicing form is filled out and the user presses enter, the form should collapse and the next form should open with the date prefilled to the submitted log events date. The exception is measurements which should be kept open since it is likely the user wants to add more measurements.

# Bug: American -> European date format and other date form fixes (Status: Planning)
We should not use american date formats mm/dd/yyyy, but rather european formats dd/mm/yyyy. If an event is created with a specific date, the form should keep that date prefilled. 

# Feature: Mark batches as failed (Status: Planning)
Sometimes a batch fails, this can be because bacteria or other reasons.

# Feature: Tasting event (Status: Planning)
When we have bottled the cider, we should be able to create a tasting event where we rate the cider and add tasting notes. The scale should be from 0-10.

# Feature: Batch statistics (Status: Planning)
Dashboard with statistics over time (such as SG over time), taste score over time etc. This should be a graph with the ability to select which data to show.
We should also be able to compare batches.

# Bug: Batch start date (Status: Planning)
When creating a new batch, the start date should not be asked for. Instead the start date should implicitly be the date of the first event in the batch.

# Feature: Batch naming
Batch ids should be prefilled with <year>-<batch letter> (e.g. 2026-A then 2026-B etc) but you should be able to override it.
