<?php

declare(strict_types=1);

function read_first_node_value(DOMNodeList $list): ?string
{
    return $list[0]->nodeValue;
}

function read_first_node(DOMNodeList $list): DOMNode|DOMNameSpaceNode|null
{
    return $list[0];
}

/**
 * @param DOMNodeList<DOMAttr> $attrs
 */
function read_first_attr_value(DOMNodeList $attrs): ?string
{
    return $attrs[0]->value;
}

function append_dom_element(DOMDocument $document, DOMElement $element): void
{
    $appended = $document->appendChild($element);
    $appended->setAttribute('data-mago', 'true');
}

function read_dom_element_name(DOMElement $element): string
{
    return $element->localName;
}

function read_dom_element_attributes(DOMElement $element): void
{
    foreach ($element->attributes as $attribute) {
        read_dom_attribute($attribute);
    }
}

function read_dom_attribute(DOMAttr $attribute): string
{
    return $attribute->name;
}
