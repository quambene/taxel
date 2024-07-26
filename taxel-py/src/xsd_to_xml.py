import pytest
from lxml import etree
from xmlschema import XMLSchema10, XMLSchemaValidationError
import json
import xml.etree.ElementTree as ET


def generate_xml(schema_path: str, data_path: str, target_namespace: str | None, namespaces: dict[str, str] | None) -> ET.ElementTree:
    print("Generate xml from schema")

    # Load and parse the XSD file
    schema = XMLSchema10(schema_path, loglevel=20, validation='strict')

    if namespaces is not None:
        if target_namespace is not None:
            namespaces[target_namespace] = schema.target_namespace

    json_file = open(data_path, 'r')
    json_data = json_file.read()
    json_data = json.loads(json_data)
    print(json_data)

    if namespaces is None:
        xml_data = schema.encode(
            json_data, validation='strict')
    else:
        xml_data = schema.encode(
            json_data, validation='strict', preserve_root=True, namespaces=namespaces)

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
    schema_path = '../test_data/simple/schema.xsd'
    input_path = '../test_data/simple/input.json'
    output_path = '../test_data/simple/output.xml'

    xml = generate_xml(schema_path, input_path, None, None)

    root = xml.getroot()
    actual_xml = ET.tostring(root, encoding='utf-8', xml_declaration=True)

    with open(output_path, 'r', encoding='utf-8') as file:
        expected_xml = file.read()

    assert actual_xml.decode("utf-8") == expected_xml


@pytest.mark.unit
def test_generate_ebilanz():
    schema_path = '../test_data/ebilanz/schema/ebilanz_000002.xsd'
    input_path = '../test_data/ebilanz/schema/input.json'
    output_path = '../test_data/ebilanz/schema/output.xml'
    target_namespace = 'ebilanz'
    namespaces = {
        'xbrli': "http://www.xbrl.org/2003/instance"}

    xml = generate_xml(schema_path, input_path, target_namespace, namespaces)

    root = xml.getroot()
    actual_xml = ET.tostring(root, encoding='utf-8', xml_declaration=True)

    with open(output_path, 'wb') as file:
        xml.write(file, encoding='utf-8', xml_declaration=True)

    # with open(output_path, 'r', encoding='utf-8') as file:
    #     expected_xml = file.read()

    # assert actual_xml.decode("utf-8") == expected_xml
