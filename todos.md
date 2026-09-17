Todo features and bugs in priority order:
When starting on each of these entries, create a docs/specs/<feature_name>.md file with a more detailed spec first for review before implementation.

# Feature: Units (Status: Done — docs/specs/units.md)
When adding additives we should select units from a dropdown list. One should be able to add to that list so we need to store it in a table. Handle existing units by adding a lowercase version of the unit to the new units table. When adding a new unit, we should check if it already exists in the table (case insensitive). If it does, we should use the existing unit instead of creating a new one.

# Feature: Easier input forms (Status: Done — docs/specs/streamlined-event-forms.md)
When for example the juicing form is filled out and the user presses enter, the form should collapse and the next form should open with the date prefilled to the submitted log events date. The exception is measurements which should be kept open since it is likely the user wants to add more measurements.

# Bug: American -> European date format and other date form fixes (Status: Done — docs/specs/european-date-format.md)
We should not use american date formats mm/dd/yyyy, but rather european formats dd/mm/yyyy. If an event is created with a specific date, the form should keep that date prefilled. 

# Feature: Mark batches as failed (Status: Done — docs/specs/batch-failure.md)
Sometimes a batch fails, this can be because bacteria or other reasons.

# Feature: Tasting event (Status: Done — docs/specs/tasting-event.md)
When we have bottled the cider, we should be able to create a tasting event where we rate the cider and add tasting notes. The scale should be from 0-10.

# Feature: Batch statistics (Status: Done — docs/specs/batch-statistics.md)
Dashboard with statistics over time (such as SG over time), taste score over time etc. This should be a graph with the ability to select which data to show.
We should also be able to compare batches.

# Bug: Batch start date (Status: Done — docs/specs/batch-start-date.md)
When creating a new batch, the start date should not be asked for. Instead the start date should implicitly be the date of the first event in the batch.

# Feature: Batch naming (Status: Done — docs/specs/batch-code-prefill.md)
Batch ids should be prefilled with <year>-<batch letter> (e.g. 2026-A then 2026-B etc) but you should be able to override it.

# Feature: Dashboard timeline (Status: Done — docs/specs/dashboard-timeline.md)
I want the dashboard to have a visualization where each batch of a season is represented as a horizontal bar on a timeline. The bar should start at the batch start date and end at the bottling date. The different batches should lie on a timeline every batch on its own row. Even though the timelines are duplicated they should represent the same time interval (some time before the start of the first batch -> some tome after the last bottling). We will call this view the Season overview. The bars should have labels according to the batch they represent and the different phases of the batch should be represented by different colors.

# Feature: Add month lines in the timeline plot (Status: Done — docs/specs/timeline-month-lines.md)
We need to be able to distinguish months in the timeline plot. This can be done by adding vertical lines for each month and labeling them with the forst three letters of the month. The lines should be in a light gray color and the labels should be in a darker gray color.

# Feature: Batch list changes (Status: Done — docs/specs/batch-list-columns.md)
I want the Batch list to have the following items:
 - Batch id
 - Start date (first event date)
 - Bottling date
 - Trees used (only a list no ratios)
 - Alcohol percentage (based on first SG measurement if SG measurement was made the same date as juicing otherwise ommited)
 - How long it has been stored since bottling (if it has been bottled)
 - A list of additives (just a list, not how much of each or ratios)

# Feature: Juicing event form changes (Status: Done — docs/specs/juicing-tree-percentage.md)
Instead of asking for kg of apples by tree, we should ask for a percentage per tree. This can be a required field. Default to an even distribution for existing entries in the database.
This way we can calculate the liters per tree based on the total output of juicing and the percentage per tree.

# Feature: Batches list should contain number of bottles if relevant (Status: Done — docs/specs/batch-list-bottle-count.md)


# Feature: Plot changes (Status: Done — docs/specs/plot-days-since-juicing.md)
We need the over-time plots to have an x axis of days after juicing rather than absolute dates. This way we can compare batches more easily. The x axis should be labeled with the number of days after juicing and the first day should be labeled as "Fermentation Start". Events should be marked as dots on the graph, hovering over them should show a summary of the event.
