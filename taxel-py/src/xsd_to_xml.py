import pytest
from lxml import etree
from xmlschema import XMLSchema, XMLSchemaValidationError
import json
import xml.etree.ElementTree as ET


def generate_xml(schema_path: str, data_path: str) -> ET.ElementTree:
    print("Generate xml from schema")

    # Load and parse the XSD file
    schema = XMLSchema(schema_path)

    json_file = open(data_path, 'r')
    json_data = json_file.read()
    json_data = json.loads(json_data)
    print(json_data)

    xml_data = schema.encode(json_data)

    if isinstance(xml_data, ET.Element):
        xml_tree = ET.ElementTree(xml_data)
    else:
        raise TypeError("Encoded data is not an XML Element")

    try:
        print("Validate data against schema")
        schema.validate(xml_tree)
        print("Validation successful")
    except XMLSchemaValidationError as err:
        print("Validation error:", err)

    return xml_tree


@pytest.mark.unit
def test_generate_xml_simple():
    schema_path = '../test_data/schema/simple/schema.xsd'
    data_path = '../test_data/schema/simple/data.json'

    xml = generate_xml(schema_path, data_path)

    root = xml.getroot()
    actual_xml = ET.tostring(root, encoding='utf-8', xml_declaration=True)

    expected_path = '../test_data/schema/simple/generated.xml'
    with open(expected_path, 'r', encoding='utf-8') as file:
        expected_xml = file.read()

    assert actual_xml.decode("utf-8") == expected_xml
