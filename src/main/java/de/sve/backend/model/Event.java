package de.sve.backend.model;

import java.time.LocalDateTime;
import java.util.List;

import com.google.auto.value.AutoValue;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

@AutoValue
@GenerateTypeAdapter
public abstract class Event {

	public abstract String id();

	public abstract String sheetId();

	public abstract Long gid();

	public abstract EventType type();

	public abstract String shortName();

	public abstract String name();

	public abstract Long sortIndex();

	public abstract Boolean visible();

	public abstract String shortDescription();

	public abstract String description();

	public abstract String image();

	public abstract String titleColor();

	public abstract List<LocalDateTime> dates();

	public abstract Integer durationInMinutes();

	public abstract Integer maxSubscribers();

	public abstract Integer subscribers();

	public abstract Double costMember();

	public abstract Double costNonMember();

	public abstract Integer waitingList();

	public abstract Integer maxWaitingList();

	public abstract String location();

	public abstract String bookingTemplate();

	public abstract String waitingTemplate();

}
