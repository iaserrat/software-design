<?php
function build($panel) {
    // Add a horizontal scroll bar
    $hScrollBar = new ScrollBar(ScrollBar::HORIZONTAL);
    $panel->add($hScrollBar);
}
